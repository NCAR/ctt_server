#![feature(let_chains)]
#![feature(addr_parse_ascii)]
mod changelog;
mod cluster;
mod conf;
mod entities;
mod migrator;
mod setup;
mod sync;
use crate::conf::Conf;
use async_graphql::{extensions::Tracing, http::GraphiQLSource, EmptySubscription, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    error_handling::HandleErrorLayer,
    extract::Extension,
    response::{self, IntoResponse},
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use cluster::RegexCluster;
use http::StatusCode;
use setup::setup_and_connect;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
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
pub(crate) use changelog::ChangeLogMsg;
use std::sync::OnceLock;

static CONFIG: OnceLock<Conf> = OnceLock::new();

#[tokio::main]
#[instrument]
async fn main() {
    // crash on panic
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    // setup config as global
    let conf_file = env::args().nth(1);
    let conf = conf::get_config(conf_file).expect("Error reading config file");
    CONFIG.set(conf.clone()).unwrap();

    // setup logging
    let stdout_log = fmt::layer().json().with_writer(std::io::stderr);
    let registry = tracing_subscriber::registry().with(
        stdout_log.with_filter(
            Targets::new()
                //sqlx logs every query at INFO
                .with_target("sqlx::query", Level::WARN)
                .with_target("cttd", Level::DEBUG)
                .with_default(Level::INFO),
        ),
    );
    tracing::subscriber::set_global_default(registry).unwrap();

    let db = Arc::new(setup_and_connect(&conf.db).await.unwrap());
    let schema = Schema::build(model::Query, model::Mutation, EmptySubscription)
        .extension(Tracing)
        .data(db.clone())
        .data(RegexCluster::new(conf.node_types.clone()))
        .finish();

    // get certificate and private key used by https
    let keys = RustlsConfig::from_pem_file(
        //PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        PathBuf::from(conf.certs_dir.clone()).join("cert.pem"),
        PathBuf::from(conf.certs_dir.clone()).join("key.pem"),
    )
    .await
    .unwrap();

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));
    tokio::spawn(sync::pbs_sync(db.clone(), conf.clone()));

    let app = Router::new()
        .route("/", get(graphiql))
        .route("/api", post(graphql_handler))
        .route_layer(Extension(schema))
        .route("/api/schema", get(schema_handler))
        .route_layer(ValidateRequestHeaderLayer::custom(conf.auth.clone()))
        //login route can't be protected by auth
        .route("/login", post(auth::login_handler))
        //add logging and timeout to all requests
        .layer(Extension(conf.clone()))
        .layer(
            ServiceBuilder::new()
                // `timeout` will produce an error if the handler takes
                // too long so we must handle those
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .layer(HandleErrorLayer::new(handle_timeout))
                .timeout(Duration::from_secs(60)),
        );

    // run https server
    let addr = SocketAddr::parse_ascii(conf.server_addr.as_bytes()).unwrap();
    axum_server::bind_rustls(addr, keys)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[instrument(skip(schema, req))]
async fn graphql_handler(
    schema: Extension<model::CttSchema>,
    Extension(role): Extension<auth::RoleGuard>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let (tx, rx) = mpsc::channel(5);
    tokio::spawn(changelog::slack_updater(rx, CONFIG.get().unwrap().clone()));
    let mut req = req.into_inner();
    req = req.data(role);
    req = req.data(tx);
    let resp = schema.execute(req).await;
    info!("{:?}", &resp);
    resp.into()
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

#[instrument]
async fn graceful_shutdown(handle: Handle) {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    select! {
        _ = sigint.recv() => (),
        _ = sigterm.recv() => (),
    };
    println!("Shutting down");
    handle.graceful_shutdown(Some(Duration::from_secs(30)));
    loop {
        sleep(Duration::from_secs(1)).await;

        println!("alive connections: {}", handle.connection_count());
    }
}
