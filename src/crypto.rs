use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::Result;
use argon2::Argon2;
use rand::RngCore;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Derive a 256-bit key from a password using Argon2id
fn derive_key(password: &str, salt: &[u8; SALT_LEN]) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow::anyhow!("Key derivation failed: {}", e))?;
    Ok(key)
}

/// Generate a random salt
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Generate a random nonce
pub fn generate_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Encrypt data using AES-256-GCM
pub fn encrypt(data: &[u8], password: &str, salt: &[u8; SALT_LEN], nonce: &[u8; NONCE_LEN]) -> Result<Vec<u8>> {
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("Failed to create cipher: {}", e))?;
    let nonce = Nonce::from_slice(nonce);

    cipher
        .encrypt(nonce, data)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))
}

/// Decrypt data using AES-256-GCM
pub fn decrypt(ciphertext: &[u8], password: &str, salt: &[u8; SALT_LEN], nonce: &[u8; NONCE_LEN]) -> Result<Vec<u8>> {
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("Failed to create cipher: {}", e))?;
    let nonce_obj = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce_obj, ciphertext)
        .map_err(|_| anyhow::anyhow!("Decryption failed: wrong key or corrupted data"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let data = b"Hello, World!";
        let password = "test_password";
        let salt = generate_salt();
        let nonce = generate_nonce();

        let encrypted = encrypt(data, password, &salt, &nonce).unwrap();
        assert_ne!(encrypted, data);

        let decrypted = decrypt(&encrypted, password, &salt, &nonce).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_wrong_password() {
        let data = b"Secret data";
        let password = "correct_password";
        let salt = generate_salt();
        let nonce = generate_nonce();

        let encrypted = encrypt(data, password, &salt, &nonce).unwrap();
        let result = decrypt(&encrypted, "wrong_password", &salt, &nonce);
        assert!(result.is_err());
    }
}
