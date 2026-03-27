use prx_voice_control::api;
use prx_voice_control::state::AppState;
use prx_voice_event::bus::{EventBus, EventBusConfig};
use prx_voice_observe::metrics::MetricsRegistry;
use std::sync::Arc;
use tokio::signal;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let event_bus = EventBus::new(EventBusConfig { capacity: 4096 });
    let metrics = Arc::new(MetricsRegistry::new());
    let state = AppState::new(event_bus, metrics);
    let app = api::router(state);

    let host = std::env::var("PRX_VOICE_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("PRX_VOICE_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let addr = format!("{host}:{port}");
    info!(%addr, "PRX Voice Engine starting");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("BUG: failed to bind listener");

    info!(%addr, "PRX Voice Engine listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("BUG: server error");

    info!("PRX Voice Engine shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("BUG: failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("BUG: failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown");
        }
        _ = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown");
        }
    }
}
