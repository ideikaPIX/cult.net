pub fn generate_peer_id(public_key_pem: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(public_key_pem.as_bytes());
    let result = hasher.finalize();
    let hex_hash = format!("{:x}", result);
    hex_hash[hex_hash.len() - 4..].to_string()
}
