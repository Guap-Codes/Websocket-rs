//! WebSocket Server Performance Benchmark Suite
//!
//! This module contains benchmarks for measuring various aspects of a WebSocket server's performance:
//! - Plaintext connection handling capacity
//! - Message processing throughput
//! - TLS handshake performance
//!
//! Key Features:
//! - Realistic simulation of client/server interactions
//! - Support for both secure (TLS) and plaintext connections
//! - Custom certificate handling for testing environments
//! - Resource management with connection limiting
//!
//! Safety Notes:
//! - The NoCertificateVerification implementation disables TLS certificate validation
//! - Should only be used for testing/benchmarking purposes
//! - Never use in production environments

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use futures_util::{SinkExt, StreamExt};
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::{net::TcpListener, runtime::Runtime, signal};
use tokio_rustls::{server::TlsStream as ServerTlsStream, client::TlsStream};
use url::Url;
use websocket_rs::{
    config::ServerConfig,
    server::{
        handler, stream::AnyStream, ClientManager, ConnectionRateLimiter, HealthMetrics, ServerState,
    },
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use rustls::{self, client::ServerCertVerified, ServerName};

use rustls::{
    client::{ServerCertVerifier},
    Certificate, Error,
};
use std::time::SystemTime;

/// Custom Certificate Verifier that bypasses certificate validation
/// 
/// # Security Warning
/// This implementation disables all certificate verification checks.
/// Only use for testing/benchmarking purposes in controlled environments.
struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    /// Always returns verification success without performing any checks
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: SystemTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }
}

/// Initializes and starts a test WebSocket server instance
/// 
/// # Arguments
/// * `enable_tls` - Whether to enable TLS encryption for the server
/// 
/// # Returns
/// Tuple containing:
/// - Server socket address
/// - Server task handle for lifecycle management
async fn start_test_server(enable_tls: bool) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let config = ServerConfig {
        port: 0,  // Let OS choose available port
        max_connections: 10_000,
        message_rate_limit: 10_000,
        enable_compression: false,
        tls_cert_path: PathBuf::from("certs/cert.pem"),
        tls_key_path: PathBuf::from("certs/key.pem"),
        enable_tls,
    };

    // Configure TLS acceptor if enabled
    let tls_acceptor = if enable_tls {
        config.create_tls_acceptor().expect("Failed to create TLS acceptor")
    } else {
        None
    };

    // Initialize shared server state
    let state = Arc::new(ServerState {
        config: Arc::new(config),
        clients: ClientManager::<AnyStream<tokio::net::TcpStream>>::new(),
        tls_acceptor,
        metrics: HealthMetrics::new(),
        rate_limiter: ConnectionRateLimiter::new(10_000),
    });

    // Bind to available port
    let listener = TcpListener::bind(format!("0.0.0.0:{}", state.config.port))
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().unwrap();

    // Clone state for shutdown handler
    let shutdown_state = state.clone();

    // Start server task with graceful shutdown support
    let server_task = tokio::spawn(async move {
        tokio::select! {
            _ = accept_connections(listener, state) => {},
            _ = shutdown_signal() => {
                shutdown_state.clients.cleanup();
            }
        }
    });

    (addr, server_task)
}

/// Main connection acceptance loop
/// 
/// # Arguments
/// * `listener` - Bound TCP listener for incoming connections
/// * `state` - Shared server state
/// 
/// Manages:
/// - Connection rate limiting with semaphore
/// - TLS handshakes for secure connections
/// - Client connection lifecycle
async fn accept_connections(
    listener: TcpListener,
    state: Arc<ServerState<AnyStream<tokio::net::TcpStream>>>,
) {
    // Connection limiter to prevent resource exhaustion
    let semaphore = Arc::new(tokio::sync::Semaphore::new(state.config.max_connections));

    loop {
        // Acquire connection permit
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        
        match listener.accept().await {
            Ok((stream, addr)) => {
                let state_clone = state.clone();
                
                // Spawn per-connection handler
                tokio::spawn(async move {
                    let _ = permit;  // Hold permit until connection ends
                    
                    if let Some(acceptor) = &state_clone.tls_acceptor {
                        // Handle TLS connections
                        match acceptor.accept(stream).await {
                            Ok(tls_stream) => {
                                let tls_state = Arc::new(ServerState {
                                    config: state_clone.config.clone(),
                                    clients: ClientManager::new(),
                                    tls_acceptor: state_clone.tls_acceptor.clone(),
                                    metrics: state_clone.metrics.clone(),
                                    rate_limiter: state_clone.rate_limiter.clone(),
                                });
                                
                                handler::handle_connection_tls(
                                    AnyStream(tls_stream),
                                    addr,
                                    tls_state
                                ).await;
                            }
                            Err(e) => eprintln!("TLS handshake failed: {}", e),
                        }
                    } else {
                        // Handle plaintext connections
                        let _ = handler::handle_connection(
                            AnyStream(stream),
                            state_clone,
                            addr
                        ).await;
                    }
                });
            }
            Err(e) => eprintln!("Accept error: {}", e),
        }
    }
}

/// Handles shutdown signal (Ctrl+C) for graceful server termination
async fn shutdown_signal() {
    signal::ctrl_c().await.expect("Failed to listen for shutdown signal");
}

/// Benchmark group for connection handling performance
/// 
/// Measures:
/// - Raw connection establishment rate
/// - Connection teardown performance
fn bench_connections(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (addr, server) = rt.block_on(start_test_server(false));

    let mut group = c.benchmark_group("connections");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(5));
    group.warm_up_time(Duration::from_secs(1));

    // Benchmark plaintext connection lifecycle
    group.bench_function("plaintext", |b| {
        b.to_async(&rt).iter(|| async {
            let url = Url::parse(&format!("ws://{}", addr)).unwrap();
            let (mut ws, _) = connect_async(url).await.unwrap();
            
            // Clean connection termination
            ws.close(None).await.unwrap();
        });
    });

    server.abort();
}

/// Benchmark group for message processing performance
/// 
/// Measures:
/// - Authentication message round-trip time
/// - Text message processing throughput
fn bench_messages(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (addr, server) = rt.block_on(start_test_server(false));

    let mut group = c.benchmark_group("messages");
    group.throughput(criterion::Throughput::Elements(1));

    // Benchmark authentication workflow
    group.bench_function("authentication", |b| {
        b.to_async(&rt).iter(|| async {
            let url = Url::parse(&format!("ws://{}", addr)).unwrap();
            let (mut ws, _) = connect_async(url).await.unwrap();
            
            ws.send(Message::Text(r#"{"type":"Auth","token":"test"}"#.into()))
                .await
                .unwrap();
            let _ = ws.next().await.unwrap().unwrap();
        });
    });

    // Benchmark text message processing
    group.bench_function("text_message", |b| {
        b.to_async(&rt).iter(|| async {
            let url = Url::parse(&format!("ws://{}", addr)).unwrap();
            let (mut ws, _) = connect_async(url).await.unwrap();
            
            let msg = Message::Text(r#"{"type":"Text","content":"test","compressed":false}"#.into());
            ws.send(msg).await.unwrap();
            let _ = ws.next().await.unwrap().unwrap();
        });
    });

    server.abort();
}

/// Benchmark group for TLS performance characteristics
/// 
/// Measures:
/// - TLS handshake establishment time
/// - Encrypted connection setup overhead
fn bench_tls(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (addr, server) = rt.block_on(start_test_server(true));

    c.bench_function("tls_handshake", |b| {
        b.to_async(&rt).iter(|| async {
            use rustls::{ClientConfig, RootCertStore};
            use tokio_rustls::TlsConnector;
            use webpki_roots;

            // Configure trust store with system roots
            let mut root_store = RootCertStore::empty();
            root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject.as_ref().to_vec(),
                    ta.subject_public_key_info.as_ref().to_vec(),
                    ta.name_constraints.as_ref().map(|nc| nc.as_ref().to_vec()),
                )
            }));
            
            // Build client TLS configuration
            let mut config = ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(root_store)
                .with_no_client_auth();

            // Bypass certificate validation (testing only!)
            config.dangerous().set_certificate_verifier(Arc::new(NoCertificateVerification));

            // Establish TLS connection
            let connector = TlsConnector::from(Arc::new(config));
            let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();

            // Perform TLS handshake
            let domain = rustls::ServerName::try_from("localhost")
                .or_else(|_| rustls::ServerName::try_from("127.0.0.1"))
                .unwrap();
            
            let tls_stream = connector.connect(domain, tcp).await.unwrap();

            // Complete WebSocket handshake
            let (_, _) = tokio_tungstenite::client_async(
                format!("wss://{}", addr),
                tls_stream
            ).await.unwrap();
        });
    });

    server.abort();
}

// Configure benchmark groups
criterion_group!(
    name = benches;
    config = Criterion::default()
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets = bench_connections, bench_messages, bench_tls
);
criterion_main!(benches);