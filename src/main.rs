use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use async_graphql::{http::GraphiQLSource, EmptyMutation, EmptySubscription, Schema, extensions::Tracing};
use tokio::signal;
use axum::{
    extract::Extension,
    response::{self, IntoResponse},
    routing::get,
    Router, Server,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
mod model;

async fn graphql_handler(
    schema: Extension<model::CttSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphiql() -> impl IntoResponse {
    response::Html(GraphiQLSource::build().endpoint("/").finish())
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let schema = Schema::build(model::Query, EmptyMutation, EmptySubscription)
        .extension(Tracing)
        .finish();

    let app = Router::new()
        .route("/", get(graphiql).post(graphql_handler))
        .layer(Extension(schema));

    println!("GraphiQL IDE: http://localhost:8000");

    Server::bind(&"127.0.0.1:8000".parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(async{signal::ctrl_c().await.unwrap()})
        .await
        .unwrap();
}

