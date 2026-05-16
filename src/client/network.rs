use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use crate::shared::{ClientMessage, ServerResponse};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct NetworkClient {
    pub sender: mpsc::UnboundedSender<ClientMessage>,
    pub receiver: Arc<Mutex<mpsc::UnboundedReceiver<ServerResponse>>>,
}

pub async fn connect(input_url: &str) -> Result<NetworkClient, Box<dyn std::error::Error>> {
    let url = if input_url.starts_with("ws://") || input_url.starts_with("wss://") {
        input_url.to_string()
    } else {
        format!("ws://{}", input_url)
    };

    let (ws_stream, _) = connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    let (tx_msg, mut rx_msg) = mpsc::unbounded_channel::<ClientMessage>();
    let (tx_resp, rx_resp) = mpsc::unbounded_channel::<ServerResponse>();

    // Send task
    tokio::spawn(async move {
        while let Some(msg) = rx_msg.recv().await {
            let json = serde_json::to_string(&msg).unwrap();
            let _ = write.send(Message::Text(json.into())).await;
        }
    });

    // Receive task
    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                if let Ok(resp) = serde_json::from_str::<ServerResponse>(&text) {
                    let _ = tx_resp.send(resp);
                }
            }
        }
    });

    Ok(NetworkClient {
        sender: tx_msg,
        receiver: Arc::new(Mutex::new(rx_resp)),
    })
}
