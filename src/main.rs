#![feature(let_chains)]
mod cluster;
mod entities;
mod migrator;
mod pbs_sync;
mod setup;
use async_graphql::{extensions::Tracing, http::GraphiQLSource, EmptySubscription, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    error_handling::HandleErrorLayer,
    extract::Extension,
    response::{self, IntoResponse},
    routing::get,
    routing::post,
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use http::StatusCode;
use setup::setup_and_connect;
#[cfg(feature = "slack")]
use slack_morphism::{
    prelude::SlackApiChatPostMessageRequest, prelude::SlackClientHyperConnector, SlackApiToken,
    SlackApiTokenValue, SlackClient, SlackMessageContent,
};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tower::ServiceBuilder;
use tower_http::validate_request::ValidateRequestHeaderLayer;
#[allow(unused_imports)]
use tracing::{info, instrument, warn, Level};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::{filter::Targets, fmt, Layer};
mod auth;
mod model;

#[instrument(skip(schema, req))]
async fn graphql_handler(
    schema: Extension<model::CttSchema>,
    Extension(role): Extension<auth::RoleGuard>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let (tx, rx) = mpsc::channel(5);
    tokio::spawn(slack_updater(rx));
    let mut req = req.into_inner();
    req = req.data(role);
    req = req.data(tx);
    let resp = schema.execute(req).await;
    info!("{:?}", &resp);
    resp.into()
}

#[cfg(not(feature = "slack"))]
#[instrument]
async fn slack_updater(mut rx: mpsc::Receiver<String>) {
    let mut updates = vec![];
    while let Some(u) = rx.recv().await {
        updates.push(u);
    }
    if updates.is_empty() {
        return;
    }
    for m in updates {
        info!(m);
    }
}

#[cfg(feature = "slack")]
#[instrument]
async fn slack_updater(mut rx: mpsc::Receiver<String>) {
    let connector = SlackClientHyperConnector::new();
    let client = SlackClient::new(connector);
    let token_value: SlackApiTokenValue =
        env::var("SLACK_TOKEN").expect("Missing SLACK_TOKEN").into();
    let token: SlackApiToken = SlackApiToken::new(token_value);
    let mut updates = vec![];
    while let Some(u) = rx.recv().await {
        updates.push(u);
    }
    if updates.is_empty() {
        return;
    }
    let session = client.open_session(&token);

    // Send a simple text message
    // TODO get channel name from config file
    let post_chat_req = SlackApiChatPostMessageRequest::new(
        "#shanks-test".into(),
        SlackMessageContent::new().with_text(format!("{:?}", updates)),
    );

    if let Err(e) = session.chat_post_message(&post_chat_req).await {
        warn!("error sending slack message {}", e);
    };
}

#[instrument]
async fn graphiql() -> impl IntoResponse {
    response::Html(GraphiQLSource::build().endpoint("/api").finish())
}

#[instrument]
async fn schema_handler() -> impl IntoResponse {
    let schema = Schema::new(model::Query, model::Mutation, EmptySubscription);
    schema.sdl()
}

#[instrument]
async fn handle_timeout(_: http::Method, _: http::Uri, _: axum::BoxError) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "request timed out".to_string(),
    )
}

#[tokio::main(flavor = "current_thread")]
#[instrument]
async fn main() {
    let stdout_log = fmt::layer().pretty().with_writer(std::io::stderr);
    let registry = tracing_subscriber::registry().with(
        stdout_log.with_filter(
            Targets::new()
                .with_target("sqlx::query", Level::WARN)
                .with_target("ctt_server", Level::DEBUG)
                .with_default(Level::INFO),
        ),
    );
    tracing::subscriber::set_global_default(registry).unwrap();

    let db = Arc::new(setup_and_connect().await.unwrap());

    tokio::spawn(pbs_sync::pbs_sync(db.clone()));
    let schema = Schema::build(model::Query, model::Mutation, EmptySubscription)
        .extension(Tracing)
        .data(db.clone())
        .finish();

    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(
        //PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        PathBuf::from("/root/shanks/ctt/")
            .join("certs")
            .join("cert.pem"),
        PathBuf::from("/root/shanks/ctt/")
            .join("certs")
            .join("key.pem"),
    )
    .await
    .unwrap();

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));

    let app = Router::new()
        .route("/", get(graphiql))
        .route("/api", post(graphql_handler))
        .route_layer(Extension(schema))
        .route("/api/schema", get(schema_handler))
        .route_layer(ValidateRequestHeaderLayer::custom(auth::Auth))
        //login route can't be protected by auth
        .route("/login", post(auth::login_handler))
        //add logging and timeout to all requests
        .layer(
            ServiceBuilder::new()
                // `timeout` will produce an error if the handler takes
                // too long so we must handle those
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .layer(HandleErrorLayer::new(handle_timeout))
                .timeout(Duration::from_secs(10)),
        );

    //info!("GraphiQL IDE: https://localhost:8000");

    // run https server
    let addr = SocketAddr::from(([10, 13, 0, 16], 8000));
    axum_server::bind_rustls(addr, config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[instrument]
async fn graceful_shutdown(handle: Handle) {
    signal::ctrl_c().await.unwrap();
    handle.graceful_shutdown(Some(Duration::from_secs(30)));
    loop {
        sleep(Duration::from_secs(1)).await;

        println!("alive connections: {}", handle.connection_count());
    }
}
