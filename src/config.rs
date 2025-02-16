use std::{fs, path::PathBuf, sync::Arc};
use serde::Deserialize;
use config::Config;
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig as RustlsServerConfig},
    TlsAcceptor,
};
use rustls_pemfile::{certs, pkcs8_private_keys};
use crate::utils::error::WebSocketError;

/// Configuration settings for the WebSocket server.
#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    /// The port on which the server will listen.
    pub port: u16,
    /// The maximum number of simultaneous connections allowed.
    pub max_connections: usize,
    /// The maximum number of messages a client can send per second.
    pub message_rate_limit: u32,
    /// Whether message compression is enabled.
    pub enable_compression: bool,
    /// Path to the TLS certificate file.
    pub tls_cert_path: PathBuf,
    /// Path to the TLS private key file.
    pub tls_key_path: PathBuf,
    /// Whether TLS is enabled for secure communication.
    pub enable_tls: bool,
}

impl ServerConfig {
    /// Loads the server configuration from environment variables.
    /// 
    /// Environment variables should be prefixed with `WS_`.
    /// 
    /// # Errors
    /// Returns a `WebSocketError::ConfigurationError` if the configuration cannot be loaded.
    pub fn from_env() -> Result<Self, WebSocketError> {
        Config::builder()
            .add_source(config::Environment::with_prefix("WS"))
            .build()
            .map_err(|e| WebSocketError::ConfigurationError(e.to_string()))?
            .try_deserialize()
            .map_err(|e| WebSocketError::ConfigurationError(e.to_string()))
    }

    /// Validates the configuration settings.
    /// 
    /// Ensures that required TLS files exist if TLS is enabled and that `max_connections` is greater than zero.
    /// 
    /// # Errors
    /// Returns a `WebSocketError::ConfigurationError` if validation fails.
    pub fn validate(&self) -> Result<(), WebSocketError> {
        if self.enable_tls {
            if self.max_connections > 10_000 {
                return Err(WebSocketError::ConfigurationError(
                    "max_connections cannot exceed 10,000".into()
                ));
            }

            if !self.tls_cert_path.exists() {
                return Err(WebSocketError::ConfigurationError(format!(
                    "Certificate file not found: {:?}", 
                    self.tls_cert_path
                )));
            }
            
            if !self.tls_key_path.exists() {
                return Err(WebSocketError::ConfigurationError(format!(
                    "Key file not found: {:?}", 
                    self.tls_key_path
                )));
            }
        }
        
        if self.max_connections == 0 {
            return Err(WebSocketError::ConfigurationError(
                "max_connections must be greater than 0".into()
            ));
        }
        
        Ok(())
    }

    /// Creates a TLS acceptor for secure WebSocket connections.
    /// 
    /// If TLS is disabled, returns `None`. Otherwise, loads the TLS certificate and private key,
    /// and initializes a Rustls TLS acceptor.
    /// 
    /// # Errors
    /// Returns a `WebSocketError::ConfigurationError` if any part of the TLS setup fails.
    pub fn create_tls_acceptor(&self) -> Result<Option<Arc<TlsAcceptor>>, WebSocketError> {
    if !self.enable_tls {
        return Ok(None);
    }

    // Load certificate chain
    let cert_chain = fs::read(&self.tls_cert_path)
        .map_err(|e| WebSocketError::ConfigurationError(format!(
            "Certificate error: {} (path: {:?})", 
            e, self.tls_cert_path
        )))?;

    // Load private key
    let key_der = fs::read(&self.tls_key_path)
        .map_err(|e| WebSocketError::ConfigurationError(format!(
            "Key error: {} (path: {:?})", 
            e, self.tls_key_path
        )))?;

    // Parse certificates
    let certs = certs(&mut cert_chain.as_slice())
        .map_err(|e| WebSocketError::ConfigurationError(format!(
            "Cert parse error: {}", e
        )))?;

    // Parse private keys
    let mut keys = pkcs8_private_keys(&mut key_der.as_slice())
        .map_err(|e| WebSocketError::ConfigurationError(format!(
            "Key parse error: {}", e
        )))?;

    // Build rustls config
    let config = RustlsServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(
            certs.into_iter().map(Certificate).collect(),
            PrivateKey(keys.remove(0)),
        )
        .map_err(|e| WebSocketError::ConfigurationError(format!(
            "TLS config error: {}", e
        )))?;

    Ok(Some(Arc::new(TlsAcceptor::from(Arc::new(config)))))
    }
}