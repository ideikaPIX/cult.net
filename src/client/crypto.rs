use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1::DecodeRsaPrivateKey, pkcs1::DecodeRsaPublicKey, pkcs1::EncodeRsaPrivateKey, pkcs1::EncodeRsaPublicKey, Pkcs1v15Encrypt};
use rand::rngs::OsRng;
use std::error::Error;

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
