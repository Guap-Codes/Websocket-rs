use futures_util::sink::SinkExt;
use tokio_tungstenite::connect_async;
use url::Url;

#[tokio::main]
async fn main() {
    let url = Url::parse("ws://127.0.0.1:8080").unwrap(); // Change to your server's URL
    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    // Send a message
    let message = tungstenite::Message::Text("Hello, WebSocket server!".to_string());
    ws_stream
        .send(message)
        .await
        .expect("Failed to send message");

    println!("Message sent to the server.");
}
