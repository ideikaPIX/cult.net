use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "action")]
pub enum ClientMessage {
    #[serde(rename = "register")]
    Register { username: String, public_key: String },
    #[serde(rename = "get_public_key")]
    GetPublicKey { target: String },
    #[serde(rename = "send_message")]
    SendMessage {
        from: String,
        to: String,
        encrypted_content: String,
        timestamp: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "status")]
pub enum ServerResponse {
    #[serde(rename = "ok")]
    RegisterOk { peer_id: String, full_address: String },
    #[serde(rename = "key_response")]
    KeyResponse { public_key: String, online_status: bool },
    #[serde(rename = "message_delivered")]
    Delivered { delivered: bool },
    #[serde(rename = "incoming_message")]
    IncomingMessage {
        from: String,
        encrypted_content: String,
    },
    #[serde(rename = "error")]
    Error { message: String },
}
