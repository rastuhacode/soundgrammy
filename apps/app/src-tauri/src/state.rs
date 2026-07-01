//! Shared application state managed by Tauri.
//!
//! A single connected grammers [`Client`] serves the one logged-in user (no
//! pooling). The client, its session, the SQLite library, config and cache
//! directories all live here behind cheap clones/locks.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use grammers_client::client::{LoginToken, PasswordToken};
use grammers_client::Client;
use grammers_session::storages::MemorySession;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::db::Db;

/// Transient authentication state kept between login command calls.
#[derive(Default)]
pub struct PendingAuth {
    /// Phone login token from `request_login_code`.
    pub phone_token: Option<LoginToken>,
    /// 2FA password token (from either phone or QR flow).
    pub password_token: Option<PasswordToken>,
    /// QR login token bytes currently displayed to the user.
    pub qr_token: Option<Vec<u8>>,
}

pub struct AppState {
    pub config: Config,
    pub db: Db,
    pub session: Arc<MemorySession>,
    pub client: Client,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    /// Guards multi-step auth flows.
    pub pending: Mutex<PendingAuth>,
    /// Per-cache-key locks so concurrent plays don't double-download a file.
    pub download_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl AppState {
    /// Returns (and lazily creates) the lock guarding a given cache key.
    pub async fn lock_for(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.download_locks.lock().await;
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Persists the current session to encrypted storage.
    pub fn persist_session(&self) -> crate::error::AppResult<()> {
        crate::session::save(&self.session, &self.data_dir)
    }
}
