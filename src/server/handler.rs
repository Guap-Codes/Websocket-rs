use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, error, info, instrument};
use tungstenite::Message;
use futures_util::{StreamExt, SinkExt};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use std::net::SocketAddr;

use crate::{
    server::{
        message::{ClientMessage, ServerMessage, MessageError},
        Client, ClientManager, ServerState
    },
    utils::error::WebSocketError,
};
use crate::server::message::{self, create_auth_response, create_error_message, create_compressed_text};
use crate::server::stream::AnyStream;


/// Handles incoming text messages from a client, parsing and processing them accordingly.
///
/// # Arguments
/// * `text` - The incoming text message.
/// * `client` - The client that sent the message.
/// * `state` - Shared state of the WebSocket server.
///
/// # Errors
/// Returns `WebSocketError` if message deserialization or processing fails.
#[instrument(skip(client, state))]
async fn handle_text<S>(
    text: String,
    client: &Client<AnyStream<S>>,
    state: &Arc<ServerState<AnyStream<S>>>,
) -> Result<(), WebSocketError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let msg: ClientMessage = serde_json::from_str(&text)
        .map_err(|e| WebSocketError::SerializationError(e.to_string()))?;

    crate::server::middleware::validation::validate_message(&msg)?;

    match msg {
        ClientMessage::Auth { token } => {
            let auth_success = authenticate(&token);
            
            let response = create_auth_response(auth_success)?;
            client.send(response)?;
            
            if auth_success {
                let mut auth_lock = client.authenticated.lock().await;
                *auth_lock = true;
            }
        }
        ClientMessage::Text { content, compressed } => {
            let response = if compressed {
                create_compressed_text(content)?
            } else {
                ServerMessage::Text(content, false).try_into()?
            };
            client.send(response)?;
        }
        ClientMessage::Command { action, parameters } => {
            debug!("Received command: {} with params: {:?}", action, parameters);
            if action == "dangerous" {
                let err = create_error_message("Dangerous commands not allowed")?;
                client.send(err)?;
            }
        }
        _ => {
            let error = create_error_message("Unexpected message format")?;
            client.send(error)?;
            return Err(MessageError::InvalidFormat.into());
        }
    }

    state.metrics.messages_sent.inc();
    Ok(())
}

/// Handles incoming binary messages, optionally decompressing them.
///
/// # Arguments
/// * `data` - The binary data received.
/// * `_client` - The client that sent the message.
/// * `state` - Shared state of the WebSocket server.
///
/// # Errors
/// Returns `WebSocketError` if message processing fails.
#[instrument(skip(_client, state))]
async fn handle_binary<S>(
    data: Vec<u8>,
    _client: &Client<AnyStream<S>>,
    state: &Arc<ServerState<AnyStream<S>>>,
) -> Result<(), WebSocketError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let msg = if state.config.enable_compression {
        let decompressed = crate::server::message::decompress_message(&data)?;
        serde_json::from_slice(&decompressed)?
    } else {
        serde_json::from_slice(&data)?
    };

    crate::server::middleware::validation::validate_message(&msg)?;

    match msg {
        ClientMessage::Binary(content, _compressed) => {
            debug!("Received binary message ({} bytes)", content.len());
        }
        _ => {
            error!("Unexpected binary message format");
            return Err(MessageError::InvalidFormat.into());
        }
    }

    state.metrics.messages_sent.inc();
    Ok(())
}

/// Authenticates a client using a provided token.
/// Returns `true` if the token is non-empty.
fn authenticate(token: &str) -> bool {
    !token.is_empty()
}


/// Handles a TLS-secured WebSocket connection, ensuring proper authentication and rate limiting.
///
/// This function wraps around `handle_connection` to handle clients connecting over TLS.
/// It first checks if the client has exceeded rate limits before proceeding to establish
/// a WebSocket session.
pub async fn handle_connection_tls(
    stream: AnyStream<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>,
    addr: std::net::SocketAddr,
    state: Arc<ServerState<AnyStream<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>>>,
) {
    if !state.rate_limiter.check(addr).await {
        error!("Rate limit exceeded for {}", addr);
        return;
    }

    // Directly pass the stream without additional wrapping
    match crate::server::handler::handle_connection(stream, state, addr).await {
        Ok(_) => info!("Connection closed: {}", addr),
        Err(e) => error!("Connection error: {}", e),
    }
}


/// Handles an incoming WebSocket connection, processing messages from the client.
///
/// # Arguments
/// * `stream` - The WebSocket stream.
/// * `state` - Shared server state.
/// * `addr` - Client's socket address.
///
/// # Errors
/// Returns `WebSocketError` if the connection fails.
#[instrument(skip(state, stream))]
pub async fn handle_connection<S>(
    stream: AnyStream<S>,
    state: Arc<ServerState<AnyStream<S>>>,
    addr: SocketAddr,
) -> Result<(), WebSocketError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|e| {
            error!("WebSocket handshake failed: {}", e);
            WebSocketError::ConnectionError(e.to_string())
        })?;

    let (writer, mut reader) = ws_stream.split();
    let client = Client::new(addr, writer);

    if !state.rate_limiter.check(addr).await {
        let error = create_error_message("Rate limit exceeded")?;
        client.send(error)?;
        client.close().await?;
        return Ok(());
    }

    state.clients.add(client.clone());
    state.metrics.connections.inc();

    let process_result = async {
        while let Some(msg) = reader.next().await {
            let msg = msg.map_err(|e| {
                error!("Read error: {}", e);
                WebSocketError::ConnectionError(e.to_string())
            })?;

            state.metrics.messages_received.inc();
            process_message(msg, &client, &state).await?;
        }
        Ok(())
    }.await;

    state.clients.remove(&client.id);
    state.metrics.connections.dec();

    client.close().await?;

    process_result
}


/// Processes different types of WebSocket messages.
///
/// # Arguments
/// * `msg` - The WebSocket message.
/// * `client` - The client that sent the message.
/// * `state` - Shared server state.
///
/// # Errors
/// Returns `WebSocketError` if message handling fails.
#[instrument(skip(client, state))]
async fn process_message<S>(
    msg: tungstenite::Message,
    client: &Client<AnyStream<S>>,
    state: &Arc<ServerState<AnyStream<S>>>,
) -> Result<(), WebSocketError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    use tungstenite::Message::*;
    
    match msg {
        Text(text) => handle_text(text, client, state).await,
        Binary(data) => handle_binary(data, client, state).await,
        Close(_) => Ok(()),
        _ => {
            debug!("Unhandled message type");
            Ok(())
        }
    }
}