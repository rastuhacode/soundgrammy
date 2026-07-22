//! Encrypted Telegram session persistence via ferogram's [`SessionBackend`].
//!
//! ferogram holds auth keys in memory and calls this backend to load/save a
//! [`PersistedSession`]. We seal the native ferogram binary snapshot with
//! AES-256-GCM and store it as `session.enc`. The 256-bit key lives in the OS
//! keychain (via `keyring`).
//!
//! Legacy grammers-shaped JSON inside `session.enc` is deleted on load (clean
//! cut) so users re-authenticate once after the library migration.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ferogram::session_backend::PersistedSession;
use ferogram::SessionBackend;

use crate::error::{AppError, AppResult};

const KEYRING_SERVICE: &str = "com.soundgrammy.app";
const KEYRING_USER: &str = "session-encryption-key";
const SESSION_FILE: &str = "session.enc";
const NONCE_LEN: usize = 12;
/// Grammers-era JSON snapshots started with `{`; ferogram binary starts with a version byte.
const LEGACY_JSON_PREFIX: u8 = b'{';

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

fn io_err(err: impl std::fmt::Display) -> io::Error {
    io::Error::other(err.to_string())
}

/// AES-GCM encrypted [`SessionBackend`] writing `session.enc` under `data_dir`.
pub struct EncryptedSessionBackend {
    path: PathBuf,
}

impl EncryptedSessionBackend {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: session_path(data_dir),
        }
    }

    pub fn arc(data_dir: &Path) -> Arc<Self> {
        Arc::new(Self::new(data_dir))
    }
}

impl SessionBackend for EncryptedSessionBackend {
    fn save(&self, session: &PersistedSession) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let plaintext = session.to_bytes();
        let cipher = cipher().map_err(io_err)?;
        let nonce = Aes256Gcm::generate_nonce(OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_slice())
            .map_err(|_| io_err("failed to encrypt session"))?;

        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(nonce.as_slice());
        blob.extend_from_slice(&ciphertext);
        std::fs::write(&self.path, blob)?;
        Ok(())
    }

    fn load(&self) -> io::Result<Option<PersistedSession>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let blob = std::fs::read(&self.path)?;
        if blob.len() <= NONCE_LEN {
            let _ = std::fs::remove_file(&self.path);
            return Ok(None);
        }

        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let cipher = match cipher() {
            Ok(c) => c,
            Err(_) => {
                let _ = std::fs::remove_file(&self.path);
                return Ok(None);
            }
        };
        let plaintext = match cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext) {
            Ok(p) => p,
            Err(_) => {
                let _ = std::fs::remove_file(&self.path);
                return Ok(None);
            }
        };

        // Clean cut: discard grammers-era JSON snapshots.
        if plaintext.first() == Some(&LEGACY_JSON_PREFIX) {
            let _ = std::fs::remove_file(&self.path);
            return Ok(None);
        }

        match PersistedSession::from_bytes(&plaintext) {
            Ok(session) => Ok(Some(session)),
            Err(_) => {
                let _ = std::fs::remove_file(&self.path);
                Ok(None)
            }
        }
    }

    fn delete(&self) -> io::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "encrypted-session.enc"
    }
}

/// Deletes the persisted session file (used on logout).
pub fn clear(data_dir: &Path) -> AppResult<()> {
    EncryptedSessionBackend::new(data_dir)
        .delete()
        .map_err(|e| AppError::msg(format!("failed to clear session: {e}")))
}
