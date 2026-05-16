use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub struct QueuedMessage {
    pub from: String,
    pub encrypted_content: String,
    pub timestamp: String,
    pub expires_at: DateTime<Utc>,
}

pub type SharedQueue = Arc<RwLock<HashMap<String, Vec<QueuedMessage>>>>;

pub fn new_queue() -> SharedQueue {
    Arc::new(RwLock::new(HashMap::new()))
}

pub async fn enqueue_message(queue: &SharedQueue, to: String, message: QueuedMessage) {
    let mut q = queue.write().await;
    q.entry(to).or_insert_with(Vec::new).push(message);
}

pub async fn get_messages(queue: &SharedQueue, to: &str) -> Vec<QueuedMessage> {
    let mut q = queue.write().await;
    let now = Utc::now();
    
    if let Some(msgs) = q.get_mut(to) {
        // Filter out expired messages
        msgs.retain(|m| m.expires_at > now);
        
        let valid_msgs = msgs.clone();
        // Clear them after retrieving
        msgs.clear();
        return valid_msgs;
    }
    Vec::new()
}
