// src/server/mod.rs
pub mod client;
pub mod handler;
pub mod health;
pub mod message;
pub mod stream;
pub mod middleware;

// Re-export public components
pub use client::{Client, ClientManager};
pub use handler::handle_connection;
pub use message::{ClientMessage, ServerMessage};
pub use health::HealthMetrics;
pub use middleware::rate_limit::ConnectionRateLimiter;

// Import internal dependencies
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;
use crate::config::ServerConfig;

#[derive(Clone)]
pub struct ServerState<S> {
    pub config: Arc<ServerConfig>,
    pub clients: ClientManager<S>,
    pub tls_acceptor: Option<Arc<TlsAcceptor>>,
    pub metrics: HealthMetrics,
    pub rate_limiter: ConnectionRateLimiter,
}