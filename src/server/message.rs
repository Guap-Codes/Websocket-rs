use serde::{Deserialize, Serialize};
use thiserror::Error;
use tungstenite::Message;
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use crate::utils::error::WebSocketError;

/// Represents different types of errors that can occur when processing messages
#[derive(Error, Debug)]
pub enum MessageError {
    /// Error when the message format is invalid.
    #[error("Invalid message format")]
    InvalidFormat,

    /// Error when an unauthorized message type is encountered.
    #[error("Unauthorized message type")]
    UnauthorizedType,
    
    /// Error when message serialization or deserialization fails.
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Error when the message size exceeds the allowed limit.
    #[error("Message too long")]
    MessageTooLong,
    
    /// Error when the binary payload size exceeds the allowed limit.
    #[error("Binary payload too large")]
    BinaryTooLarge,
    
    /// Error when compression fails.
    #[error("Compression error: {0}")]
    CompressionError(String),
    
    /// Error when decompression fails.
    #[error("Decompression error: {0}")]
    DecompressionError(String),
}

/// Represents the types of messages a client can send to the server.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    /// Authentication message containing a token.
    Auth { token: String },

    /// A text message, which may be compressed.
    Text { content: String, compressed: bool },

    /// A command message with an action and parameters.
    Command { action: String, parameters: Vec<String> },

    /// A binary message with an optional compression flag.
    Binary(Vec<u8>, bool),  // Added compression flag
}

/// Represents messages that the server sends to the client.
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Indicates the result of an authentication attempt.
    AuthResult(bool),

    /// A text message, which may be compressed.
    Text(String, bool),      

    /// A notification message sent to the client.
    Notification(String),

    /// An error message describing a failure.
    Error(String),

    /// A binary message, always compressed.
    Binary(Vec<u8>),          
}

/// Converts a Tungstenite `Message` into a `ClientMessage`, handling text and binary data.
impl TryFrom<Message> for ClientMessage {
    type Error = MessageError;

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        match msg {
            Message::Text(text) => serde_json::from_str(&text)
                .map_err(|e| MessageError::SerializationError(e.to_string())),
            
            Message::Binary(data) => {
                // Attempt decompression first
                match decompress_message(&data) {
                    Ok(decompressed) => serde_json::from_slice(&decompressed)
                        .map(|msg| match msg {
                            ClientMessage::Text { content, .. } => 
                                ClientMessage::Text { content, compressed: true },
                            ClientMessage::Binary(_, _) => 
                                ClientMessage::Binary(decompressed, true),
                            other => other,
                        })
                        .map_err(|e| MessageError::SerializationError(e.to_string())),
                    
                    Err(_) => Ok(ClientMessage::Binary(data, false)),
                }
            }
            
            _ => Err(MessageError::InvalidFormat),
        }
    }
}

/// Converts a `ServerMessage` into a Tungstenite `Message`, handling serialization and compression.
impl TryFrom<ServerMessage> for Message {
    type Error = WebSocketError;

    fn try_from(msg: ServerMessage) -> Result<Self, Self::Error> {
        match msg {
            ServerMessage::Text(text, compressed) if compressed => {
                let compressed = compress_message(text.as_bytes())
                    .map_err(|e| WebSocketError::from(e))?;
                Ok(Message::Binary(compressed))
            }
            
            ServerMessage::Text(text, _) => {
                Ok(Message::Text(text))
            }
            
            ServerMessage::Binary(data) => {
                Ok(Message::Binary(data))
            }
            
            other => {
                let json = serde_json::to_string(&other)
                    .map_err(|e| WebSocketError::SerializationError(e.to_string()))?;
                Ok(Message::Text(json))
            }
        }
    }
}

/// Compresses a given byte slice using the default compression level.
pub fn compress_message(data: &[u8]) -> Result<Vec<u8>, MessageError> {
    let mut compressor = Compress::new(Compression::default(), false);
    let mut output = Vec::with_capacity(data.len() / 2);
    
    compressor.compress_vec(data, &mut output, FlushCompress::Finish)
        .map_err(|e| MessageError::CompressionError(e.to_string()))?;
    
    if output.is_empty() {
        Err(MessageError::CompressionError("Compressed data empty".into()))
    } else {
        Ok(output)
    }
}

/// Decompresses a given byte slice, handling buffer errors and stream completion.
pub fn decompress_message(data: &[u8]) -> Result<Vec<u8>, MessageError> {
    let mut decompressor = Decompress::new(false);
    let mut output = Vec::with_capacity(data.len() * 2);
    
    let status = decompressor.decompress_vec(data, &mut output, FlushDecompress::Finish)
        .map_err(|e| MessageError::DecompressionError(e.to_string()))?;
    
    match status {
        Status::Ok => Ok(output),
        Status::BufError => Err(MessageError::DecompressionError("Buffer error".into())),
        Status::StreamEnd => Ok(output),
        _ => Err(MessageError::DecompressionError("Unknown error".into())),
    }
}

/// Creates a WebSocket message indicating the authentication result.
pub fn create_auth_response(success: bool) -> Result<Message, WebSocketError> {
    let response = ServerMessage::AuthResult(success);
    response.try_into()
}

/// Creates a WebSocket error message with a given error string.
pub fn create_error_message(error: &str) -> Result<Message, WebSocketError> {
    let response = ServerMessage::Error(error.to_string());
    response.try_into()
}

/// Creates a compressed text WebSocket message.
pub fn create_compressed_text(text: String) -> Result<Message, WebSocketError> {
    let response = ServerMessage::Text(text, true);
    response.try_into()
}