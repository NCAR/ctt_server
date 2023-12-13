#![feature(let_chains)]
mod cluster;
use std::collections::HashMap;
mod entities;
use entities::target::TargetStatus;
mod migrator;
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
use pbs::{Attrl, Op};
use setup::setup_and_connect;
#[cfg(feature = "slack")]
use slack_morphism::{
    prelude::SlackApiChatPostMessageRequest, prelude::SlackClientHyperConnector, SlackApiToken,
    SlackApiTokenValue, SlackClient, SlackMessageContent,
};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time::{self, sleep};
use tower::ServiceBuilder;
use tower_http::validate_request::ValidateRequestHeaderLayer;
#[allow(unused_imports)]
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
mod auth;
mod model;
use sea_orm::{DatabaseConnection, QuerySelect};

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

    // Send a simple text messagele
    let post_chat_req = SlackApiChatPostMessageRequest::new(
        "#shanks-test".into(),
        SlackMessageContent::new().with_text(format!("{:?}", updates)),
    );

    if let Err(e) = session.chat_post_message(&post_chat_req).await {
        warn!("error sending slack message {}", e);
    };
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
        "request timed out".to_string(),
    )
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder().finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let db = setup_and_connect().await.unwrap();

    let schema = Schema::build(model::Query, model::Mutation, EmptySubscription)
        .extension(Tracing)
        .data(db.clone())
        .finish();

    // configure certificate and private key used by https
    let config = RustlsConfig::from_pem_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("certs")
            .join("cert.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("certs")
            .join("key.pem"),
    )
    .await
    .unwrap();

    let handle = Handle::new();
    tokio::spawn(graceful_shutdown(handle.clone()));
    tokio::spawn(pbs_sync(db));

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
    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    axum_server::bind_rustls(addr, config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn pbs_sync(db: DatabaseConnection) {
    //TODO get interval from config file
    let mut interval = time::interval(Duration::from_secs(30));
    // don't let ticks stack up if a sync takes longer than interval
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    loop {
        let pbs_srv = pbs::Server::new();
        //TODO filter stat attribs (just need hostname, jobs, and state)
        //TODO need to handle err
        let _pbs_node_state: HashMap<String, TargetStatus> = pbs_srv
            .stat_vnode(&None, None)
            .unwrap()
            .resources
            .iter()
            .map(|n| {
                let name = n.name();
                let jobs = match n.attribs().get("jobs").unwrap() {
                    Attrl::Value(Op::Default(j)) => j.is_empty(),
                    x => {
                        println!("{:?}", x);
                        panic!("bad job list");
                    }
                };
                let state = match n.attribs().get("state").unwrap() {
                    Attrl::Value(Op::Default(j)) => j,
                    x => {
                        println!("{:?}", x);
                        panic!("bad state");
                    }
                };
                let state = match state.as_str() {
                    x if x.contains("unknown") => TargetStatus::Unknown,
                    //job-excl or resv-excl
                    x if x.contains("exclusive") => TargetStatus::Online,
                    //order matters, before "down" to capture down,offline nodes
                    x if x.contains("offline") => {
                        if jobs {
                            TargetStatus::Draining
                        } else {
                            TargetStatus::Offline
                        }
                    }
                    "down" => {
                        if jobs {
                            TargetStatus::Draining
                        } else {
                            TargetStatus::Down
                        }
                    }
                    "job-busy" => TargetStatus::Online,
                    "free" => TargetStatus::Online,
                    x => {
                        warn!("Unrecognized node state, '{}'", x);
                        TargetStatus::Unknown
                    }
                };
                (name, state)
            })
            .collect();
        let ctt_node_state = entities::target::Entity::all()
            .select_only()
            .columns([
                entities::target::Column::Name,
                entities::target::Column::Status,
            ])
            .all(&db)
            .await
            .unwrap();
        let _ctt_node_state: HashMap<String, TargetStatus> = ctt_node_state
            .iter()
            .map(|n| (n.name.clone(), n.status))
            .collect();
        // sync ctt and pbs

        interval.tick().await;
    }
}

async fn graceful_shutdown(handle: Handle) {
    signal::ctrl_c().await.unwrap();
    handle.graceful_shutdown(Some(Duration::from_secs(30)));
    loop {
        sleep(Duration::from_secs(1)).await;

        println!("alive connections: {}", handle.connection_count());
    }
}
