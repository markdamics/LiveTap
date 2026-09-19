mod routes;
mod session;

use std::{net::SocketAddr, time::Duration};

use axum::{
    routing::{any, get, post},
    Router,
};
use session::SessionStore;
use tower_http::trace::TraceLayer;

const DEFAULT_SESSION_TTL_SECS: u64 = 3600;
const SWEEP_INTERVAL_SECS: u64 = 30;

#[derive(Clone)]
pub struct AppState {
    store: SessionStore,
    public_base_url: String,
    public_ws_base_url: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let addr = std::env::var("LIVETAP_RELAY_ADDR").unwrap_or_else(|_| "0.0.0.0:8787".to_string());

    let ttl_secs = std::env::var("LIVETAP_SESSION_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SESSION_TTL_SECS);
    let store = SessionStore::new(Duration::from_secs(ttl_secs));

    let public_base_url = std::env::var("LIVETAP_PUBLIC_BASE_URL")
        .unwrap_or_else(|_| format!("http://{}", addr.replace("0.0.0.0", "127.0.0.1")));
    let public_ws_base_url = to_ws_scheme(&public_base_url);

    let state = AppState {
        store: store.clone(),
        public_base_url,
        public_ws_base_url,
    };

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(SWEEP_INTERVAL_SECS));
        loop {
            interval.tick().await;
            store.sweep_expired();
        }
    });

    let app = Router::new()
        .route("/health", get(routes::health))
        .route("/session", post(routes::create_session))
        .route("/hook/{session_id}", any(routes::capture_hook))
        .route("/hook/{session_id}/{*rest}", any(routes::capture_hook))
        .route("/ws/{session_id}", get(routes::ws_upgrade))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|err| panic!("failed to bind {addr}: {err}"));
    tracing::info!("livetap-relay listening on {addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("relay server error");
}

fn to_ws_scheme(base_url: &str) -> String {
    if let Some(rest) = base_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base_url.to_string()
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}
