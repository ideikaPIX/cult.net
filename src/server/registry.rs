use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct RegistryEntry {
    pub public_key: String,
    pub is_online: bool,
    pub last_seen: Option<String>,
    pub trusted_peers: HashSet<String>,
}

pub type SharedRegistry = Arc<RwLock<HashMap<String, RegistryEntry>>>;

pub fn new_registry() -> SharedRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

pub async fn register_user(registry: &SharedRegistry, full_address: String, public_key: String) {
    let mut reg = registry.write().await;
    let entry = reg.entry(full_address).or_insert_with(|| RegistryEntry {
        public_key: public_key.clone(),
        is_online: true,
        last_seen: None,
        trusted_peers: HashSet::new(),
    });
    entry.public_key = public_key;
    entry.is_online = true;
}

pub async fn set_online_status(registry: &SharedRegistry, full_address: &str, is_online: bool) {
    let mut reg = registry.write().await;
    if let Some(entry) = reg.get_mut(full_address) {
        entry.is_online = is_online;
        if !is_online {
            entry.last_seen = Some(chrono::Utc::now().to_rfc3339());
        }
    }
}

pub async fn get_public_key(registry: &SharedRegistry, full_address: &str) -> Option<(String, bool)> {
    let reg = registry.read().await;
    reg.get(full_address).map(|e| (e.public_key.clone(), e.is_online))
}

pub async fn add_trust(registry: &SharedRegistry, address_a: &str, address_b: &str) {
    let mut reg = registry.write().await;
    if let Some(entry_a) = reg.get_mut(address_a) {
        entry_a.trusted_peers.insert(address_b.to_string());
    }
    if let Some(entry_b) = reg.get_mut(address_b) {
        entry_b.trusted_peers.insert(address_a.to_string());
    }
}

pub async fn get_status(registry: &SharedRegistry, target: &str, requester: &str) -> Option<(bool, Option<String>)> {
    let reg = registry.read().await;
    
    if let Some(target_entry) = reg.get(target) {
        let mut last_seen = None;
        
        // Проверяем взаимность: запрашивал ли requester ключ target, И запрашивал ли target ключ requester
        let requester_intents_target = target_entry.trusted_peers.contains(requester);
        let target_intents_requester = reg.get(requester)
            .map(|req_entry| req_entry.trusted_peers.contains(target))
            .unwrap_or(false);

        if requester_intents_target && target_intents_requester {
            last_seen = target_entry.last_seen.clone();
        }
        
        Some((target_entry.is_online, last_seen))
    } else {
        None
    }
}
