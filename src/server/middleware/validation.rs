use crate::server::message::ClientMessage;
use crate::server::message::MessageError;

/// Validates a client message to ensure it meets size constraints.
///
/// # Arguments
///
/// * `msg` - A reference to the `ClientMessage` that needs validation.
///
/// # Returns
///
/// * `Ok(())` if the message is within allowed limits.
/// * `Err(MessageError::MessageTooLong)` if a text message exceeds 1024 characters.
/// * `Err(MessageError::BinaryTooLarge)` if a binary message exceeds 4096 bytes.
pub fn validate_message(msg: &ClientMessage) -> Result<(), MessageError> {
    match msg {
        ClientMessage::Text { content, .. } if content.len() > 1024 => {
            Err(MessageError::MessageTooLong)
        }
        ClientMessage::Binary(data, ..) if data.len() > 4096 => Err(MessageError::BinaryTooLarge),
        _ => Ok(()),
    }
}
