//! Encrypted, persistent Telegram session — no pooling.
//!
//! grammers' `MemorySession` holds the auth key in memory. We snapshot its
//! serializable [`SessionData`] (home DC + per-DC options carrying the auth
//! key), seal it with AES-256-GCM, and write it to disk. The 256-bit key lives
//! in the OS keychain (via `keyring`) and is generated on first run. This
//! satisfies "session restored from a ciphered local machine".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use grammers_session::storages::MemorySession;
use grammers_session::Session;
use grammers_session::SessionData;

use crate::error::{AppError, AppResult};

const KEYRING_SERVICE: &str = "com.soundgrammy.app";
const KEYRING_USER: &str = "session-encryption-key";
const SESSION_FILE: &str = "session.enc";
const NONCE_LEN: usize = 12;
/// Telegram currently exposes five primary datacenters.
const DC_ID_RANGE: std::ops::RangeInclusive<i32> = 1..=5;

/// Minimal, serde-serializable snapshot of the parts of a session we persist:
/// the home DC and the per-DC options (which carry the permanent auth key).
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedSession {
    home_dc: i32,
    dc_options: Vec<grammers_session::types::DcOption>,
}

fn session_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SESSION_FILE)
}

/// Loads the AES key from the OS keychain, generating and storing a new random
/// one on first run.
fn load_or_create_key() -> AppResult<[u8; 32]> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::msg(format!("keychain unavailable: {e}")))?;

    match entry.get_password() {
        Ok(encoded) => {
            let bytes = B64
                .decode(encoded.as_bytes())
                .map_err(|e| AppError::msg(format!("corrupt session key: {e}")))?;
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| AppError::msg("session key has unexpected length"))?;
            Ok(arr)
        }
        Err(keyring::Error::NoEntry) => {
            let key = Aes256Gcm::generate_key(OsRng);
            entry
                .set_password(&B64.encode(key.as_slice()))
                .map_err(|e| AppError::msg(format!("failed to persist session key: {e}")))?;
            let arr: [u8; 32] = key.as_slice().try_into().unwrap();
            Ok(arr)
        }
        Err(e) => Err(AppError::msg(format!("keychain error: {e}"))),
    }
}

fn cipher() -> AppResult<Aes256Gcm> {
    let key_bytes = load_or_create_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    Ok(Aes256Gcm::new(key))
}

/// Loads the persisted session (decrypting it) or creates a fresh empty one.
pub fn load_or_create(data_dir: &Path) -> AppResult<Arc<MemorySession>> {
    let path = session_path(data_dir);
    if !path.exists() {
        return Ok(Arc::new(MemorySession::default()));
    }

    let blob = std::fs::read(&path)?;
    if blob.len() <= NONCE_LEN {
        return Ok(Arc::new(MemorySession::default()));
    }

    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let cipher = cipher()?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| AppError::msg("failed to decrypt session (key mismatch?)"))?;

    let persisted: PersistedSession = match serde_json::from_slice(&plaintext) {
        Ok(p) => p,
        Err(_) => return Ok(Arc::new(MemorySession::default())),
    };

    let mut data = SessionData {
        home_dc: persisted.home_dc,
        ..SessionData::default()
    };
    for option in persisted.dc_options {
        data.dc_options.insert(option.id, option);
    }

    Ok(Arc::new(MemorySession::from(data)))
}

/// Encrypts and writes the current session to disk. Only the home DC and DC
/// options (auth keys) are persisted; peer/update caches are rebuilt at runtime.
pub fn save(session: &MemorySession, data_dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(data_dir)?;

    let dc_options = DC_ID_RANGE
        .filter_map(|dc_id| session.dc_option(dc_id))
        .collect::<Vec<_>>();
    let persisted = PersistedSession {
        home_dc: session.home_dc_id(),
        dc_options,
    };
    let plaintext = serde_json::to_vec(&persisted)?;

    let cipher = cipher()?;
    let nonce = Aes256Gcm::generate_nonce(OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_slice())
        .map_err(|_| AppError::msg("failed to encrypt session"))?;

    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(nonce.as_slice());
    blob.extend_from_slice(&ciphertext);
    std::fs::write(session_path(data_dir), blob)?;
    Ok(())
}

/// Deletes the persisted session file (used on logout).
pub fn clear(data_dir: &Path) -> AppResult<()> {
    let path = session_path(data_dir);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
