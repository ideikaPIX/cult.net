use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::tungstenite::Message;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::registry::SharedRegistry;
use crate::queue::SharedQueue;
use crate::shared::{ClientMessage, ServerResponse};
use crate::registry;
use crate::queue;
use crate::auth;
use chrono::{Utc, Duration};

type Tx = mpsc::UnboundedSender<Message>;
type SharedClients = Arc<RwLock<HashMap<String, Tx>>>;

pub async fn start_server(addr: &str, registry: SharedRegistry, queue: SharedQueue) {
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    println!("Listening on: {}", addr);
    
    let clients: SharedClients = Arc::new(RwLock::new(HashMap::new()));

    while let Ok((stream, _)) = listener.accept().await {
        let registry_clone = registry.clone();
        let queue_clone = queue.clone();
        let clients_clone = clients.clone();
        
        tokio::spawn(async move {
            handle_connection(stream, registry_clone, queue_clone, clients_clone).await;
        });
    }
}

async fn handle_connection(stream: TcpStream, registry: SharedRegistry, queue: SharedQueue, clients: SharedClients) {
    let peer_addr = stream.peer_addr().ok();
    let ws_stream = accept_async(stream).await.expect("Error during the websocket handshake occurred");
    println!("New client connected: {:?}", peer_addr);
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut current_user_address: Option<String> = None;

    let clients_clone = clients.clone();
    
    // Task to send messages to the client
    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let _ = ws_sender.send(message).await;
        }
    });

    // Task to receive messages from the client
    while let Some(msg) = ws_receiver.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };

        if msg.is_text() {
            let text = msg.to_text().unwrap();
            println!("Raw message received: {}", text); // Log raw message
            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(text) {
                match client_msg {
                    ClientMessage::Register { username, public_key } => {
                        let peer_id = auth::generate_peer_id(&public_key);
                        let full_address = format!("{}#{}@cult.net", username, peer_id);
                        
                        registry::register_user(&registry, full_address.clone(), public_key).await;
                        
                        let resp = ServerResponse::RegisterOk {
                            peer_id,
                            full_address: full_address.clone(),
                        };
                        
                        clients.write().await.insert(full_address.clone(), tx.clone());
                        current_user_address = Some(full_address.clone());
                        registry::set_online_status(&registry, &full_address, true).await;
                        
                        let _ = tx.send(Message::Text(serde_json::to_string(&resp).unwrap().into()));

                        // Flush pending messages
                        let pending = queue::get_messages(&queue, &full_address).await;
                        for pm in pending {
                            let msg_resp = ServerResponse::IncomingMessage {
                                from: pm.from,
                                encrypted_content: pm.encrypted_content,
                            };
                            let _ = tx.send(Message::Text(serde_json::to_string(&msg_resp).unwrap().into()));
                        }
                    },
                    ClientMessage::GetPublicKey { target } => {
                        if let Some(from) = current_user_address.as_ref() {
                            registry::add_trust(&registry, from, &target).await;
                        }
                        if let Some((pub_key, is_online)) = registry::get_public_key(&registry, &target).await {
                            let resp = ServerResponse::KeyResponse {
                                public_key: pub_key,
                                online_status: is_online,
                            };
                            let _ = tx.send(Message::Text(serde_json::to_string(&resp).unwrap().into()));
                        } else {
                            let resp = ServerResponse::Error { message: "Not found".to_string() };
                            let _ = tx.send(Message::Text(serde_json::to_string(&resp).unwrap().into()));
                        }
                    },
                    ClientMessage::CheckStatus { target } => {
                        if let Some(from) = current_user_address.as_ref() {
                            let (online, last_seen) = if let Some((is_online, ls)) = registry::get_status(&registry, &target, from).await {
                                (is_online, ls)
                            } else {
                                (false, None)
                            };
                            let resp = ServerResponse::StatusResponse {
                                target,
                                online,
                                last_seen,
                            };
                            let _ = tx.send(Message::Text(serde_json::to_string(&resp).unwrap().into()));
                        }
                    },
                    ClientMessage::SendMessage { from, to, encrypted_content, timestamp } => {
                        // Very basic auth check: ensure 'from' matches current_user_address
                        if current_user_address.as_ref() == Some(&from) {
                            println!("Forwarding message from {} to {}", from, to);
                            let clients_read = clients.read().await;
                            if let Some(target_tx) = clients_read.get(&to) {
                                // Target is online
                                let msg_resp = ServerResponse::IncomingMessage {
                                    from: from.clone(),
                                    encrypted_content,
                                };
                                let _ = target_tx.send(Message::Text(serde_json::to_string(&msg_resp).unwrap().into()));
                                
                                // Send delivery receipt
                                let resp = ServerResponse::Delivered { delivered: true };
                                let _ = tx.send(Message::Text(serde_json::to_string(&resp).unwrap().into()));
                            } else {
                                // Target is offline, enqueue
                                let qm = queue::QueuedMessage {
                                    from: from.clone(),
                                    encrypted_content,
                                    timestamp,
                                    expires_at: Utc::now() + Duration::hours(24),
                                };
                                queue::enqueue_message(&queue, to.clone(), qm).await;
                                println!("Target {} offline, message enqueued", to);
                            }
                        }
                    },
                    ClientMessage::Disconnect => {
                        if let Some(ref current_user) = current_user_address {
                            println!("Пользователь {} запросил отключение", current_user);
                        }
                        break;
                    }
                }
            }
        }
    }

    send_task.abort();
    
    // КРИТИЧЕСКИЙ ФИКС: Очистка реестра после выхода из цикла (по Disconnect или по обрыву связи)
    if let Some(address) = current_user_address {
        println!("Удаляем {} из глобального реестра онлайна", address);
        registry::set_online_status(&registry, &address, false).await;
        clients_clone.write().await.remove(&address);
    }
}
