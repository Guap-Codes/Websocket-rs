use prometheus::{Encoder, IntCounter, IntGauge, Registry, TextEncoder};
use axum::{Router, routing::get, extract::State, response::IntoResponse};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use axum_server::tls_rustls::RustlsConfig;
use tower::util::ServiceExt;

use crate::server::ServerState;

/// Struct for managing server health metrics, including active connections and message counts.
#[derive(Clone)]
pub struct HealthMetrics {
    /// Tracks the number of active WebSocket connections.
    pub connections: IntGauge,
    /// Counts the total number of messages received by the server.
    pub messages_received: IntCounter,
    /// Counts the total number of messages sent by the server.
    pub messages_sent: IntCounter,
    /// Prometheus registry used to store and manage the metrics.
    registry: Registry,
}

impl HealthMetrics {
    /// Creates a new instance of `HealthMetrics` and registers the metrics with Prometheus.
    /// 
    /// # Returns
    /// A `HealthMetrics` instance with initialized metrics.
    pub fn new() -> Self {
        let registry = Registry::new();
        let connections = IntGauge::new("connections", "Active connections").unwrap();
        let messages_received = IntCounter::new("messages_received", "Total messages received").unwrap();
        let messages_sent = IntCounter::new("messages_sent", "Total messages sent").unwrap();

        registry.register(Box::new(connections.clone())).unwrap();
        registry.register(Box::new(messages_received.clone())).unwrap();
        registry.register(Box::new(messages_sent.clone())).unwrap();

        Self { 
            connections, 
            messages_received, 
            messages_sent, 
            registry 
        }
    }

    /// Exposes the current state of all registered metrics in Prometheus-compatible format.
    /// 
    /// # Returns
    /// A `String` containing the encoded metrics data.
    pub fn expose_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

/// Starts an HTTP server to expose the metrics on port 9080.
/// 
/// This function creates an Axum router that serves the `/metrics` endpoint, 
/// which provides Prometheus-compatible metrics data.
/// 
/// # Arguments
/// * `state` - A shared reference to the `ServerState`, containing metrics and other server state.
/// 
/// # Type Parameters
/// * `S` - A generic type that implements `AsyncRead`, `AsyncWrite`, `Unpin`, `Send`, and `Sync`.
pub async fn serve_metrics_http<S>(state: Arc<ServerState<S>>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:9080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}


// Starts an HTTPS server to expose the metrics on port 9090.
/// 
/// This function sets up a secure TLS connection using Rustls and serves the `/metrics` endpoint.
/// 
/// # Arguments
/// * `state` - A shared reference to the `ServerState`, containing metrics and other server state.
/// 
/// # Type Parameters
/// * `S` - A generic type that implements `AsyncRead`, `AsyncWrite`, `Unpin`, `Send`, and `Sync`.
/*pub async fn serve_metrics_https<S>(state: Arc<ServerState<S>>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    // Create a separate router for metrics without WebSocket dependencies
    let metrics_router = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let config = RustlsConfig::from_pem_file(
        "certs/cert.pem",
        "certs/key.pem"
    )
    .await
    .unwrap();

    // Convert to a service using axum::serve compatibility
    let service = metrics_router.into_make_service();

    axum_server::bind_rustls(
        "0.0.0.0:9090".parse().unwrap(),
        config
    )
    .serve(service)
    .await
    .unwrap();
}*/

/// Handles the `/metrics` HTTP request and returns the current metrics data.
/// 
/// This function retrieves the metrics from `ServerState` and formats them in a way
/// that Prometheus can scrape and process.
/// 
/// # Arguments
/// * `state` - Extracted Axum state containing the `ServerState` instance.
/// 
/// # Returns
/// A response containing Prometheus-formatted metrics data.
async fn metrics_handler<S>(
    State(state): State<Arc<ServerState<S>>>
) -> impl IntoResponse 
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    state.metrics.expose_metrics()
}

