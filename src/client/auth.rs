use crate::client::crypto;
use crate::client::storage::{self, Account};
use sha2::{Sha256, Digest};
use chrono::Utc;

pub fn generate_peer_id(public_key_pem: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key_pem.as_bytes());
    let result = hasher.finalize();
    let hex_hash = format!("{:x}", result);
    // Return last 4 characters
    hex_hash[hex_hash.len() - 4..].to_string()
}

pub fn get_active_account() -> Result<Option<Account>, Box<dyn std::error::Error>> {
    let data = storage::load_accounts()?;
    Ok(data.accounts.into_iter().find(|a| a.is_active))
}

pub fn login(username: &str, is_online: bool) -> Result<Account, Box<dyn std::error::Error>> {
    storage::ensure_dirs()?;

    let keypair = crypto::generate_keypair()?;
    let peer_id = generate_peer_id(&keypair.public_key_pem);
    let full_address = format!("{}#{}@cult.net", username, peer_id);

    let account = Account {
        username: username.to_string(),
        peer_id,
        full_address,
        public_key: keypair.public_key_pem,
        private_key: keypair.private_key_pem,
        created_at: Utc::now().to_rfc3339(),
        is_active: true,
        confirmed_online: is_online, // if online registration succeeded
        vps_confirmed: is_online,
    };

    let mut data = storage::load_accounts()?;
    
    // Set all other accounts to inactive
    for a in &mut data.accounts {
        a.is_active = false;
    }
    
    data.accounts.push(account.clone());
    storage::save_accounts(&data)?;

    Ok(account)
}

pub fn switch_account(peer_address: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let mut data = storage::load_accounts()?;
    let mut found = false;
    for a in &mut data.accounts {
        if a.full_address == peer_address {
            a.is_active = true;
            found = true;
        } else {
            a.is_active = false;
        }
    }
    
    if found {
        storage::save_accounts(&data)?;
    }
    Ok(found)
}
