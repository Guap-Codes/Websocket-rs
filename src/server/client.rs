use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};
use uuid::Uuid;
use dashmap::DashMap;
use tokio::sync::mpsc;
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, error, instrument};
use tungstenite::Message;
use futures_util::SinkExt;
use tokio_tungstenite::WebSocketStream;
use tokio::io::{AsyncRead, AsyncWrite};
use crate::utils::WebSocketError;
use futures_util::stream::{SplitSink, SplitStream};



/// Represents a connected client with an associated WebSocket connection.
#[derive(Debug)]
pub struct Client<S> {
    /// Unique identifier for the client.
    pub id: Uuid,
    /// Socket address of the client.
    pub addr: SocketAddr,
    /// Authentication status of the client.
    pub authenticated: Arc<TokioMutex<bool>>,
    /// Timestamp of the client's last activity.
    pub last_activity: Mutex<Instant>,
    /// Channel sender for sending messages to the client.
    sender: mpsc::UnboundedSender<Message>,
    /// Guard to track active connections.
    connection_guard: Arc<()>,
    /// Phantom data to associate with the generic type `S`.
    _phantom: std::marker::PhantomData<S>,
}


impl<S> Clone for Client<S> {
    fn clone(&self) -> Self {
        let last_activity = self.last_activity.lock().unwrap();
        Self {
            id: self.id,
            addr: self.addr,
            authenticated: self.authenticated.clone(),
            last_activity: Mutex::new(*last_activity),
            sender: self.sender.clone(),
            connection_guard: self.connection_guard.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}


impl<S> Client<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    /// Creates a new `Client` instance with the given socket address and WebSocket writer.
    /// 
    /// # Arguments
    /// 
    /// * `addr` - The socket address of the client.
    /// * `writer` - A split sink WebSocket writer for sending messages.
    #[instrument(skip(writer))]
    pub fn new(
        addr: SocketAddr,
        writer: futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    ) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let connection_guard = Arc::new(());

        tokio::spawn({
            let guard = connection_guard.clone();
            async move {
                let mut writer = writer;
                while let Some(msg) = receiver.recv().await {
                    if let Err(e) = writer.send(msg).await {
                        error!("Failed to send message: {}", e);
                        break;
                    }
                }
                drop(guard);
            }
        });

        Client {
            id: Uuid::new_v4(),
            addr,
            authenticated: Arc::new(TokioMutex::new(false)),
            last_activity: Mutex::new(Instant::now()),
            sender,
            connection_guard,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Sends a message to the client.
    /// 
    /// # Arguments
    /// 
    /// * `message` - The WebSocket message to send.
    /// 
    /// # Errors
    /// 
    /// Returns `ClientError::SendFailed` if the message cannot be sent.
    #[instrument(skip(self))]
    pub fn send(&self, message: Message) -> Result<(), ClientError> {
        {
            let mut last_activity = self.last_activity.lock().unwrap();
            *last_activity = Instant::now();
        }
        self.sender.send(message)
            .map_err(|e| {
                error!("Failed to queue message: {}", e);
                ClientError::SendFailed
            })
    }

    /// Checks if the client is still connected.
    /// 
    /// # Returns
    /// 
    /// `true` if the client is connected, otherwise `false`.
    pub fn is_connected(&self) -> bool {
        Arc::strong_count(&self.connection_guard) > 1
    }

    pub async fn close(&self) -> Result<(), WebSocketError> {
        self.sender.send(Message::Close(None))
            .map_err(|_| WebSocketError::ConnectionError("Failed to send close frame".into()))
    }

}

/// Manages multiple client connections.
pub struct ClientManager<S> {
    /// A concurrent map storing active clients.
    clients: Arc<DashMap<Uuid, Client<S>>>,
}

// Manual Clone implementation
impl<S> Clone for ClientManager<S> {
    fn clone(&self) -> Self {
        ClientManager {
            clients: Arc::clone(&self.clients),
        }
    }
}

impl<S> ClientManager<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    /// Creates a new `ClientManager` instance.
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
        }
    }

    /// Adds a new client to the manager.
    /// 
    /// # Arguments
    /// 
    /// * `client` - The client instance to add.
    pub fn add(&self, client: Client<S>) {
        self.clients.insert(client.id, client);
    }

    /// Removes a client from the manager by ID.
    /// 
    /// # Arguments
    /// 
    /// * `id` - The unique identifier of the client to remove.
    pub fn remove(&self, id: &Uuid) {
        self.clients.remove(id);
    }

    /// Cleans up disconnected clients from the manager.
    pub fn cleanup(&self) {
        self.clients.retain(|_, client| {
            let connected = client.is_connected();
            if !connected {
                debug!("Removing disconnected client: {}", client.id);
            }
            connected
        });
    }

    /// Broadcasts a message to all connected clients.
    /// 
    /// # Arguments
    /// 
    /// * `message` - The WebSocket message to broadcast.
    #[instrument(skip(self))]
    pub fn broadcast(&self, message: Message) {
        self.clients.iter().for_each(|entry| {
            if let Err(e) = entry.value().send(message.clone()) {
                error!("Broadcast failed to {}: {}", entry.key(), e);
                self.clients.remove(entry.key());
            }
        });
    }
}


/// Represents errors that may occur in client operations.
#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Failed to send message")]
    SendFailed,
    #[error("Client not found")]
    NotFound,
}