use axum::{Router, routing::get};
use prometheus::Encoder;
use tokio::net::TcpListener;
use tokio::sync::{broadcast::Receiver, mpsc::Sender};
use tracing::instrument;

const API_BIND_ADDR: &str = "127.0.0.1:9091";

pub async fn run_api_server(
    _shutdown_complete_tx: Sender<()>,
    mut notify_shutdown_rx: Receiver<()>,
) {
    let app = Router::<()>::new().route("/metrics", get(metrics_handler));
    let listener: TcpListener = TcpListener::bind(API_BIND_ADDR).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = notify_shutdown_rx.recv().await;
        })
        .await
        .unwrap();
}

#[instrument(level = "trace")]
async fn metrics_handler() -> String {
    let mut buffer = Vec::new();
    let encoder = prometheus::TextEncoder::new();

    let metric_families = prometheus::gather();
    encoder
        .encode(&metric_families, &mut buffer)
        .expect("Failed to encode metrics");

    String::from_utf8(buffer).expect("Metrics should be valid UTF-8")
}
