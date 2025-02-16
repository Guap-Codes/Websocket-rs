use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use tokio_tungstenite::connect_async;
use tungstenite::protocol::Message;

#[tokio::test]
async fn test_websocket_connection() {
    let (ws_stream, _) = connect_async("ws://127.0.0.1:8080").await.unwrap();
    let (mut write, mut read) = ws_stream.split();

    write
        .send(Message::Text("Hello".to_string()))
        .await
        .unwrap();
    let msg = read.next().await.unwrap().unwrap();

    assert_eq!(msg, Message::Text("Hello".to_string()));
}
