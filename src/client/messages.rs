use crate::shared::{ClientMessage, ServerResponse};
use crate::client::crypto;
use crate::client::storage;
use crate::client::network::NetworkClient;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn send_message(
    net: &NetworkClient,
    from_address: &str,
    to_address: &str,
    to_public_key: &str,
    plaintext: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let encrypted_content = crypto::encrypt(plaintext, to_public_key)?;
    let timestamp = Utc::now().to_rfc3339();

    let msg = ClientMessage::SendMessage {
        from: from_address.to_string(),
        to: to_address.to_string(),
        encrypted_content: encrypted_content.clone(),
        timestamp: timestamp.clone(),
    };

    net.sender.send(msg)?;

    // Save locally as pending
    let conn = storage::get_chat_db(to_address)?;
    conn.execute(
        "INSERT INTO messages (timestamp, sender, content, status, is_yours) VALUES (?1, ?2, ?3, ?4, ?5)",
        (timestamp, from_address, plaintext, "pending", true),
    )?;

    Ok(())
}

pub async fn handle_incoming(
    receiver: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<ServerResponse>>>,
    private_key_pem: String,
) {
    let mut rx = receiver.lock().await;
    while let Some(resp) = rx.recv().await {
        match resp {
            ServerResponse::IncomingMessage { from, encrypted_content } => {
                if let Ok(plaintext) = crypto::decrypt(&encrypted_content, &private_key_pem) {
                    if let Ok(conn) = storage::get_chat_db(&from) {
                        let timestamp = Utc::now().to_rfc3339();
                        let _ = conn.execute(
                            "INSERT INTO messages (timestamp, sender, content, status, is_yours) VALUES (?1, ?2, ?3, ?4, ?5)",
                            (timestamp, &from, &plaintext, "delivered", false),
                        );
                    }
                }
            }
            ServerResponse::Delivered { .. } => {
                // In a real implementation we would correlate this with a message ID
                // For now, we might update the last pending message to delivered
            }
            _ => {}
        }
    }
}
