//! Encrypted Telegram session persistence via ferogram's [`SessionBackend`].
//!
//! ferogram holds auth keys in memory and calls this backend to load/save a
//! [`PersistedSession`]. We seal the native ferogram binary snapshot with
//! AES-256-GCM and store it as `session.enc`. The 256-bit key lives in the OS
//! keychain (via `keyring`).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aes_gcm::aead::{Aead, Generate, Key, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ferogram::session_backend::PersistedSession;
use ferogram::SessionBackend;

use crate::error::{AppError, AppResult};

const KEYRING_SERVICE: &str = "com.soundgrammy.app";
const KEYRING_USER: &str = "session-encryption-key";
const SESSION_FILE: &str = "session.enc";
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

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
            let key = Key::<Aes256Gcm>::generate();
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
    let key = Key::<Aes256Gcm>::try_from(key_bytes.as_slice())
        .map_err(|_| AppError::msg("session key has unexpected length"))?;
    Ok(Aes256Gcm::new(&key))
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

    fn load_with_cipher<F>(&self, load_cipher: F) -> io::Result<Option<PersistedSession>>
    where
        F: FnOnce() -> io::Result<Aes256Gcm>,
    {
        let blob = match std::fs::read(&self.path) {
            Ok(blob) => blob,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if blob.len() < NONCE_LEN + TAG_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "encrypted session is truncated: expected at least {} bytes, got {}",
                    NONCE_LEN + TAG_LEN,
                    blob.len()
                ),
            ));
        }

        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let cipher = load_cipher()?;
        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid encrypted session nonce",
            )
        })?;
        let plaintext = cipher.decrypt(&nonce, ciphertext).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to decrypt session: authentication failed",
            )
        })?;

        PersistedSession::from_bytes(&plaintext)
            .map(Some)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to decode persisted session: {e}"),
                )
            })
    }
}

impl SessionBackend for EncryptedSessionBackend {
    fn save(&self, session: &PersistedSession) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let plaintext = session.to_bytes();
        let cipher = cipher().map_err(io_err)?;
        let nonce = Nonce::generate();
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
        self.load_with_cipher(|| cipher().map_err(io_err))
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

/// Whether a non-trivial encrypted session file is on disk (no network, no decrypt).
///
/// Used as the local “logged in” gate together with a SQLite profile row.
pub fn exists(data_dir: &Path) -> bool {
    let path = session_path(data_dir);
    match std::fs::metadata(&path) {
        Ok(meta) => meta.is_file() && meta.len() > NONCE_LEN as u64,
        Err(_) => false,
    }
}

/// Deletes the persisted session file (used on logout).
pub fn clear(data_dir: &Path) -> AppResult<()> {
    EncryptedSessionBackend::new(data_dir)
        .delete()
        .map_err(|e| AppError::msg(format!("failed to clear session: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("soundgrammy-session-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_cipher(byte: u8) -> Aes256Gcm {
        let key_bytes = [byte; 32];
        let key = Key::<Aes256Gcm>::try_from(key_bytes.as_slice()).unwrap();
        Aes256Gcm::new(&key)
    }

    fn encrypted_blob(cipher: &Aes256Gcm, plaintext: &[u8]) -> Vec<u8> {
        let nonce_bytes = [7u8; NONCE_LEN];
        let nonce = Nonce::try_from(nonce_bytes.as_slice()).unwrap();
        let ciphertext = cipher.encrypt(&nonce, plaintext).unwrap();
        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        blob
    }

    fn assert_load_error_preserves_file<F>(
        backend: &EncryptedSessionBackend,
        expected_blob: &[u8],
        load_cipher: F,
    ) -> io::Error
    where
        F: FnOnce() -> io::Result<Aes256Gcm>,
    {
        let error = backend.load_with_cipher(load_cipher).unwrap_err();
        assert_eq!(fs::read(&backend.path).unwrap(), expected_blob);
        error
    }

    #[test]
    fn missing_session_file_returns_none() {
        let dir = test_dir("missing");
        let backend = EncryptedSessionBackend::new(&dir);

        assert!(backend.load().unwrap().is_none());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn key_store_failure_preserves_session_file() {
        let dir = test_dir("key-store-failure");
        let backend = EncryptedSessionBackend::new(&dir);
        let blob = vec![1u8; NONCE_LEN + 16];
        fs::write(&backend.path, &blob).unwrap();

        let error = assert_load_error_preserves_file(&backend, &blob, || {
            Err(io::Error::other("keychain temporarily unavailable"))
        });
        assert!(error
            .to_string()
            .contains("keychain temporarily unavailable"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn truncated_ciphertext_preserves_session_file() {
        let dir = test_dir("truncated");
        let backend = EncryptedSessionBackend::new(&dir);
        let blob = vec![1u8; NONCE_LEN + TAG_LEN - 1];
        fs::write(&backend.path, &blob).unwrap();

        let error = assert_load_error_preserves_file(&backend, &blob, || Ok(test_cipher(1)));
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("truncated"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn authentication_failure_preserves_session_file() {
        let dir = test_dir("authentication-failure");
        let backend = EncryptedSessionBackend::new(&dir);
        let blob = encrypted_blob(&test_cipher(1), &PersistedSession::default().to_bytes());
        fs::write(&backend.path, &blob).unwrap();

        let error = assert_load_error_preserves_file(&backend, &blob, || Ok(test_cipher(2)));
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("authentication failed"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_session_plaintext_preserves_session_file() {
        let dir = test_dir("invalid-plaintext");
        let backend = EncryptedSessionBackend::new(&dir);
        let blob = encrypted_blob(&test_cipher(1), b"not a persisted session");
        fs::write(&backend.path, &blob).unwrap();

        let error = assert_load_error_preserves_file(&backend, &blob, || Ok(test_cipher(1)));
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error
            .to_string()
            .contains("failed to decode persisted session"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn explicit_delete_removes_session_file() {
        let dir = test_dir("explicit-delete");
        let backend = EncryptedSessionBackend::new(&dir);
        fs::write(&backend.path, [1u8; NONCE_LEN + 1]).unwrap();

        backend.delete().unwrap();

        assert!(!backend.path.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn exists_requires_non_trivial_session_file() {
        let dir = test_dir("exists");

        assert!(!exists(&dir));

        fs::write(session_path(&dir), [0u8; NONCE_LEN]).unwrap();
        assert!(!exists(&dir), "nonce-only file is not a session");

        fs::write(session_path(&dir), [0u8; NONCE_LEN + 1]).unwrap();
        assert!(exists(&dir));

        clear(&dir).unwrap();
        assert!(!exists(&dir));

        let _ = fs::remove_dir_all(&dir);
    }
}
