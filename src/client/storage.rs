use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use rusqlite::{Connection, Result as SqliteResult};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub username: String,
    pub peer_id: String,
    pub full_address: String,
    pub public_key: String,
    pub private_key: String,
    pub created_at: String,
    pub is_active: bool,
    pub confirmed_online: bool,
    pub vps_confirmed: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct AccountsData {
    pub accounts: Vec<Account>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Contact {
    pub username: String,
    pub peer_id: String,
    pub full_address: String,
    pub public_key: String,
    pub added_at: String,
    pub last_message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ContactsData {
    pub contacts: Vec<Contact>,
}

pub fn get_cult_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".cult");
    path
}

pub fn ensure_dirs() -> std::io::Result<()> {
    let base = get_cult_dir();
    fs::create_dir_all(base.join("authinfo"))?;
    fs::create_dir_all(base.join("chats"))?;
    fs::create_dir_all(base.join("logs"))?;
    Ok(())
}

pub fn load_accounts() -> Result<AccountsData, Box<dyn std::error::Error>> {
    let path = get_cult_dir().join("authinfo").join("accounts.json");
    if !path.exists() {
        return Ok(AccountsData::default());
    }
    let content = fs::read_to_string(path)?;
    let data = serde_json::from_str(&content)?;
    Ok(data)
}

pub fn save_accounts(data: &AccountsData) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_cult_dir().join("authinfo").join("accounts.json");
    let content = serde_json::to_string_pretty(data)?;
    fs::write(&path, content)?;
    
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }
    
    Ok(())
}

pub fn load_contacts() -> Result<ContactsData, Box<dyn std::error::Error>> {
    let path = get_cult_dir().join("chats").join("contacts.json");
    if !path.exists() {
        return Ok(ContactsData::default());
    }
    let content = fs::read_to_string(path)?;
    let data = serde_json::from_str(&content)?;
    Ok(data)
}

pub fn save_contacts(data: &ContactsData) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_cult_dir().join("chats").join("contacts.json");
    let content = serde_json::to_string_pretty(data)?;
    fs::write(&path, content)?;
    Ok(())
}

#[derive(Debug)]
pub struct MessageRecord {
    pub id: i64,
    pub timestamp: String,
    pub sender: String,
    pub content: String,
    pub status: String,
    pub is_yours: bool,
}

pub fn get_chat_db(peer_address: &str) -> SqliteResult<Connection> {
    let db_name = format!("{}.db", peer_address);
    let path = get_cult_dir().join("chats").join(db_name);
    let conn = Connection::open(path)?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY,
            timestamp TEXT NOT NULL,
            sender TEXT NOT NULL,
            content TEXT NOT NULL,
            status TEXT NOT NULL,
            is_yours BOOLEAN NOT NULL
        )",
        [],
    )?;
    
    Ok(conn)
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ServerHistory {
    pub ips: Vec<String>,
}

pub fn load_server_history() -> Result<ServerHistory, Box<dyn std::error::Error>> {
    let path = get_cult_dir().join("authinfo").join("servers.json");
    if !path.exists() {
        return Ok(ServerHistory::default());
    }
    let content = fs::read_to_string(path)?;
    let data = serde_json::from_str(&content)?;
    Ok(data)
}

pub fn save_server_history(data: &ServerHistory) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_cult_dir().join("authinfo").join("servers.json");
    let content = serde_json::to_string_pretty(data)?;
    fs::write(&path, content)?;
    Ok(())
}

