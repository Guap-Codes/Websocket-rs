use thiserror::Error;

/// Represents various errors that can occur in a WebSocket server.
#[derive(Error, Debug)]
pub enum WebSocketError {
    /// Represents a general connection error.
    /// 
    /// This error occurs when the WebSocket connection fails or encounters issues.
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Indicates that authentication has failed.
    ///
    /// This can happen if the provided credentials or authentication token is invalid.
    #[error("Authentication failed")]
    AuthenticationError,
    
    /// Indicates that the client is not authorized to perform a specific action.
    ///
    /// This error is triggered when access control checks fail.
    #[error("Authorization failed")]
    AuthorizationError,
    
    /// Indicates that the client has exceeded the allowed request rate.
    ///
    /// Used for enforcing rate limits and preventing abuse.
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    /// Represents an error related to WebSocket message handling.
    ///
    /// This error occurs when messages are malformed or fail to be processed.
    #[error("Message error: {0}")]
    MessageError(#[from] crate::server::message::MessageError),
    
    /// Indicates a failure in serializing or deserializing data.
    ///
    /// This can happen when converting messages to or from JSON.
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Represents an error in the server configuration.
    ///
    /// This occurs when an invalid or inconsistent configuration is detected.
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    /// Indicates that data validation has failed.
    ///
    /// This can occur when user input or message content does not meet expected criteria.
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    /// Represents an error related to client operations.
    ///
    /// This includes issues such as unexpected disconnections or client misbehavior.
    #[error("Client error: {0}")]
    ClientError(#[from] crate::server::client::ClientError),

    /// Indicates a failure when mutating the client state.
    ///
    /// This may happen if an operation on a connected client encounters an unexpected issue.
    #[error("Client mutation error")]
    ClientMutationError,
}


/// Implements conversion from `serde_json::Error` to `WebSocketError`.
///
/// This allows serialization errors to be automatically converted into
/// `WebSocketError::SerializationError`.
impl From<serde_json::Error> for WebSocketError {
    fn from(err: serde_json::Error) -> Self {
        WebSocketError::SerializationError(err.to_string())
    }
}