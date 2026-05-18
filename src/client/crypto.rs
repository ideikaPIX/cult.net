use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1::DecodeRsaPrivateKey, pkcs1::DecodeRsaPublicKey, pkcs1::EncodeRsaPrivateKey, pkcs1::EncodeRsaPublicKey, Pkcs1v15Encrypt};
use rand::rngs::OsRng;
use std::error::Error;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum InnerPayload {
    KeyInit { encrypted_aes_key: Vec<u8> },
    SecureText { ciphertext: Vec<u8>, nonce: Vec<u8> },
}

pub struct KeyPair {
    pub private_key_pem: String,
    pub public_key_pem: String,
}

pub fn generate_keypair() -> Result<KeyPair, Box<dyn Error>> {
    let mut rng = OsRng;
    let bits = 2048;
    let priv_key = RsaPrivateKey::new(&mut rng, bits)?;
    let pub_key = RsaPublicKey::from(&priv_key);

    let private_key_pem = priv_key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)?.to_string();
    let public_key_pem = pub_key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)?.to_string();

    Ok(KeyPair {
        private_key_pem,
        public_key_pem,
    })
}

use base64::{Engine as _, engine::general_purpose};

pub fn encrypt(plaintext: &str, public_key_pem: &str) -> Result<String, Box<dyn Error>> {
    let pub_key = RsaPublicKey::from_pkcs1_pem(public_key_pem)?;
    let mut rng = OsRng;
    let enc_data = pub_key.encrypt(&mut rng, Pkcs1v15Encrypt, plaintext.as_bytes())?;
    Ok(general_purpose::STANDARD.encode(enc_data))
}

pub fn decrypt(ciphertext_base64: &str, private_key_pem: &str) -> Result<String, Box<dyn Error>> {
    let priv_key = RsaPrivateKey::from_pkcs1_pem(private_key_pem)?;
    let enc_data = general_purpose::STANDARD.decode(ciphertext_base64)?;
    let dec_data = priv_key.decrypt(Pkcs1v15Encrypt, &enc_data)?;
    Ok(String::from_utf8(dec_data)?)
}

pub fn aes_encrypt(text: &str, key: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>), Box<dyn Error>> {
    use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};
    use rand::RngCore;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, text.as_bytes())
        .map_err(|e| format!("AES Encryption error: {:?}", e))?;
    
    Ok((ciphertext, nonce_bytes.to_vec()))
}

pub fn aes_decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8]) -> Result<String, Box<dyn Error>> {
    use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce_obj = Nonce::from_slice(nonce);

    let plaintext = cipher.decrypt(nonce_obj, ciphertext)
        .map_err(|e| format!("AES Decryption error: {:?}", e))?;

    Ok(String::from_utf8(plaintext)?)
}
