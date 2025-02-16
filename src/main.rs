//! # Secure TCP/TLS Websocket
//! 
//! This module implements a secure TCP/TLS websocket with support for rate limiting,
//! health metrics, and graceful shutdown.
//! 
//! ## Features
//! - Secure TLS support using `tokio-rustls`
//! - Rate limiting for incoming connections
//! - Graceful shutdown handling
//! - Environment-based configuration loading
//! - Health monitoring via HTTP metrics endpoint
//! 
//! ## Dependencies
//! - `tokio` for asynchronous runtime
//! - `tokio-rustls` for TLS support
//! - `dotenv` for environment configuration
//! - `tracing` for logging



use websocket_rs::{config, server, utils};
use std::sync::Arc;
use tokio::{net::TcpListener, signal};
use tracing::{error, info};
use tokio_rustls::server::TlsStream;
use tokio::net::TcpStream;
use websocket_rs::server::{ServerState, ClientManager, stream::AnyStream};


/// Entry point for the websocket application.
/// 
/// Initializes logging, loads configuration from the environment,
/// and starts the TCP/TLS listener.
/// 
/// # Errors
/// Returns an error if configuration validation fails or if the server fails to bind to a port.
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let _ = dotenv::dotenv();
    tracing_subscriber::fmt::init();

    let config = config::ServerConfig::from_env()?;
    config.validate()?;

    // Create state with wrapped TCP stream type
    let state = Arc::new(ServerState::<AnyStream<TcpStream>> {
        config: Arc::new(config.clone()),
        clients: server::ClientManager::new(),
        tls_acceptor: config.create_tls_acceptor()?,
        metrics: server::health::HealthMetrics::new(),
        rate_limiter: server::middleware::rate_limit::ConnectionRateLimiter::new(
            config.message_rate_limit
        ),
    });

    // Start listening on the configured port
    let listener = TcpListener::bind(format!("0.0.0.0:{}", state.config.port)).await?;
    info!("Server listening on port {}", state.config.port);

    tokio::spawn(server::health::serve_metrics_http(state.clone()));
//    tokio::spawn(server::health::serve_metrics_https(state.clone()));

    // Handle incoming connections or shutdown signals
    tokio::select! {
        _ = accept_connections(listener, state.clone()) => {},
        _ = shutdown_signal() => {
            info!("Shutting down gracefully");
            state.clients.cleanup();
        }
    }

    Ok(())
}

/// Accepts and handles incoming TCP connections.
/// 
/// This function continuously listens for new connections and spawns a new
/// task to process each connection. If TLS is enabled, the connection is
/// upgraded using `tokio-rustls` before being handled.
/// 
/// # Arguments
/// - `listener`: The TCP listener that accepts new connections.
/// - `state`: Shared server state used for managing connections.
async fn accept_connections(
    listener: TcpListener,
    state: Arc<ServerState<AnyStream<TcpStream>>>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    match &state_clone.tls_acceptor {
                        Some(acceptor) => {
                            match acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    // Create wrapped TLS stream state
                                    let tls_state = Arc::new(ServerState {
                                        config: state_clone.config.clone(),
                                        clients: server::ClientManager::new(),
                                        tls_acceptor: state_clone.tls_acceptor.clone(),
                                        metrics: state_clone.metrics.clone(),
                                        rate_limiter: state_clone.rate_limiter.clone(),
                                    });
                                    
                                    server::handler::handle_connection_tls(
                                        AnyStream(tls_stream),
                                        addr,
                                        tls_state
                                    ).await;
                                }
                                Err(e) => error!("TLS handshake failed: {}", e),
                            }
                        }
                        None => {
                     let _ = server::handler::handle_connection(
                                AnyStream(stream),
                                state_clone,
                                addr
                            ).await;
                        }
                    }
                });
            }
            Err(e) => error!("Accept error: {}", e),
        }
    }
}

/// Listens for a shutdown signal (Ctrl+C) and initiates a graceful shutdown.
/// 
/// This function blocks until the signal is received, allowing the server
/// to perform cleanup before exiting.
async fn shutdown_signal() {
    signal::ctrl_c().await.expect("Failed to listen for shutdown signal");
}