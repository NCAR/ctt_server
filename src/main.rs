use async_graphql::{extensions::Tracing, http::GraphiQLSource, EmptySubscription, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use serde_json::json;
use axum::{
    error_handling::HandleErrorLayer,
    extract::Extension,
    http::header::{self, HeaderMap},
    response::{self, IntoResponse},
    routing::get,
    routing::post,
    Router, Server,
};
use jsonwebtoken::{encode, EncodingKey, Header, DecodingKey, Validation, decode, Algorithm};
use chrono::{NaiveDateTime, Utc};
use http::{StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use tower_http::validate_request::ValidateRequest;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use http_body::combinators::UnsyncBoxBody;
use http_body::Body;
use axum::body::BoxBody;
use axum::extract;
mod model;

const SKETCHY_SECRET: &str = "6e313fae4b113e12c469edb558ccc92e331751efd5441c031802b04441efa7a3";

#[derive(Clone, Copy)]
struct Auth;

impl<B> ValidateRequest<B> for Auth
{
    type ResponseBody = axum::body::BoxBody;

    fn validate(&mut self, request: &mut axum::http::Request<B>) -> axum::response::Result<(), axum::response::Response> {
            if let Some(user) = check_auth(&request) {
                // Set `user_id` as a request extension so it can be accessed by other
                // services down the stack.
                info!("Request validated for user {}", &user.user);
                request.extensions_mut().insert(user);

                Ok(())
            } else {
                let unauthorized_response: axum::response::Response = axum::response::Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(BoxBody::default())
                    .unwrap();
                info!("Invalid request");

                Err(unauthorized_response)
            }
    }
}

fn check_auth<B>(request: &axum::http::Request<B>) -> Option<model::RoleGuard> {
    request.headers().get(header::AUTHORIZATION)
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_value| {
            if auth_value.starts_with("Bearer ") {
                Some(auth_value[7..].to_owned())
            } else {
                None
            }
        })
        .and_then(|t| Some(decode::<model::RoleGuard>(&t, &DecodingKey::from_base64_secret(SKETCHY_SECRET).unwrap(), &Validation::new(Algorithm::HS256)).unwrap()))
        .and_then(|c| Some(c.claims))

    //Some(model::RoleGuard::new(model::Role::Admin, "shanks".to_string(), Utc::now().naive_utc()))
}

#[derive(Deserialize, Debug)]
struct UserLogin {
    user: String,
    timestamp: NaiveDateTime,
}

async fn login_handler(extract::Json(payload): extract::Json<UserLogin>) -> Result<axum::Json<String>,(StatusCode, String)> {
    info!("Login request: {:?}", payload);
    if payload.user != "shanks" {
        Err((StatusCode::FORBIDDEN, "User not authorized".to_string()))
    } else {
        let key = EncodingKey::from_base64_secret(SKETCHY_SECRET).unwrap();
        let claims = model::RoleGuard::new(model::Role::Admin,"shanks".to_string(), Utc::now().naive_utc()+chrono::Duration::minutes(6000));
        let token = encode(
            &Header::default(),
            &claims,
            &key,
        ).unwrap();
        Ok(axum::Json(json!({"token": token}).to_string())) 
    }
}
async fn graphql_handler(
    schema: Extension<model::CttSchema>,
    _headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphiql() -> impl IntoResponse {
    response::Html(GraphiQLSource::build().endpoint("/api").finish())
}

async fn schema_handler() -> impl IntoResponse {
    let schema = Schema::new(model::Query, model::Mutation, EmptySubscription);
    schema.sdl()
}

async fn handle_timeout(_: http::Method, _: http::Uri, _: axum::BoxError) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("request timed out"),
    )
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let schema = Schema::build(model::Query, model::Mutation, EmptySubscription)
        .extension(Tracing)
        .finish();

    let app = Router::new()
        .route("/", get(graphiql))
        .route("/api", post(graphql_handler))
        .route_layer(Extension(schema))
        .route("/api/schema", get(schema_handler))
        .route_layer(ValidateRequestHeaderLayer::custom(Auth))
        //login route can't be protected by auth
        .route("/login", post(login_handler))
        //add logging and timeout to all requests
        .layer(
            ServiceBuilder::new()
                // `timeout` will produce an error if the handler takes
                // too long so we must handle those
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .layer(HandleErrorLayer::new(handle_timeout))
                .timeout(Duration::from_secs(10)),
        );

    info!("GraphiQL IDE: http://localhost:8000");

    Server::bind(&"127.0.0.1:8000".parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(async { signal::ctrl_c().await.unwrap() })
        .await
        .unwrap();
}
