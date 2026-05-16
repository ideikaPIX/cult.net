use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct RegistryEntry {
    pub public_key: String,
    pub is_online: bool,
}

pub type SharedRegistry = Arc<RwLock<HashMap<String, RegistryEntry>>>;

pub fn new_registry() -> SharedRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

pub async fn register_user(registry: &SharedRegistry, full_address: String, public_key: String) {
    let mut reg = registry.write().await;
    reg.insert(full_address, RegistryEntry {
        public_key,
        is_online: true, // Assuming registration happens while online
    });
}

pub async fn set_online_status(registry: &SharedRegistry, full_address: &str, is_online: bool) {
    let mut reg = registry.write().await;
    if let Some(entry) = reg.get_mut(full_address) {
        entry.is_online = is_online;
    }
}

pub async fn get_public_key(registry: &SharedRegistry, full_address: &str) -> Option<(String, bool)> {
    let reg = registry.read().await;
    reg.get(full_address).map(|e| (e.public_key.clone(), e.is_online))
}
