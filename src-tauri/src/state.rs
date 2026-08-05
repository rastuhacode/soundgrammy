//! Shared application state managed by Tauri.
//!
//! A ferogram [`Client`] may be absent at startup when Telegram is unreachable
//! (blocked network / misconfigured proxy). The login UI can still load so the
//! user can enable an MTProto proxy and reconnect.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{atomic::AtomicI64, Arc};

use ferogram::{Client, LoginToken, PasswordToken, ShutdownToken};
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::proxy_settings::{self, ProxySettings};
use crate::streaming::StreamingManager;
use crate::telegram;

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

struct LiveTelegram {
    client: Client,
    shutdown: ShutdownToken,
}

pub struct AppState {
    pub config: Config,
    pub db: Db,
    telegram: RwLock<Option<LiveTelegram>>,
    /// Serializes client rebuilds (proxy apply / disable).
    reconnect_lock: Mutex<()>,
    /// Whether the live client was built with an MTProto proxy.
    proxy_active: Mutex<bool>,
    /// Last proxy/connect error shown in settings / login UI.
    proxy_last_error: Mutex<Option<String>>,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    /// Guards multi-step auth flows.
    pub pending: Mutex<PendingAuth>,
    /// Serializes saved-music reconciliation across automatic and manual syncs.
    pub sync_lock: Mutex<()>,
    /// Per-cache-key locks so concurrent plays don't double-download a file.
    pub download_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// Per-track locks deduplicate profile requests; the slot keeps CPU-heavy
    /// decoding to one track at a time.
    pub bounce_analysis_locks: Mutex<HashMap<i64, Arc<Mutex<()>>>>,
    pub bounce_analysis_slot: Mutex<()>,
    pub bounce_requested_track: AtomicI64,
    /// Active progressive audio downloads, shared by playback and explicit saves.
    pub streaming: StreamingManager,
}

impl AppState {
    pub fn new(
        config: Config,
        db: Db,
        client: Option<(Client, ShutdownToken)>,
        data_dir: PathBuf,
        cache_dir: PathBuf,
        proxy_active: bool,
        proxy_last_error: Option<String>,
    ) -> Self {
        let telegram = client.map(|(client, shutdown)| LiveTelegram { client, shutdown });
        Self {
            config,
            db,
            telegram: RwLock::new(telegram),
            reconnect_lock: Mutex::new(()),
            proxy_active: Mutex::new(proxy_active),
            proxy_last_error: Mutex::new(proxy_last_error),
            data_dir,
            cache_dir,
            pending: Default::default(),
            sync_lock: Default::default(),
            download_locks: Default::default(),
            bounce_analysis_locks: Default::default(),
            bounce_analysis_slot: Default::default(),
            bounce_requested_track: AtomicI64::new(-1),
            streaming: Default::default(),
        }
    }

    /// Cheap clone of the live ferogram client handle.
    pub async fn client(&self) -> AppResult<Client> {
        let guard = self.telegram.read().await;
        match guard.as_ref() {
            Some(live) => Ok(live.client.clone()),
            None => {
                let detail = self
                    .proxy_last_error
                    .lock()
                    .await
                    .clone()
                    .unwrap_or_else(|| "not connected to Telegram".into());
                Err(AppError::msg(format!(
                    "telegram offline: {detail}. Enable an MTProto proxy (or VPN) and apply it."
                )))
            }
        }
    }

    pub async fn is_telegram_online(&self) -> bool {
        self.telegram.read().await.is_some()
    }

    pub async fn proxy_active(&self) -> bool {
        *self.proxy_active.lock().await
    }

    pub async fn proxy_last_error(&self) -> Option<String> {
        self.proxy_last_error.lock().await.clone()
    }

    pub async fn set_proxy_last_error(&self, error: Option<String>) {
        *self.proxy_last_error.lock().await = error;
    }

    /// Returns (and lazily creates) the lock guarding a given cache key.
    pub async fn lock_for(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.download_locks.lock().await;
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub async fn bounce_lock_for(&self, track_id: i64) -> Arc<Mutex<()>> {
        let mut locks = self.bounce_analysis_locks.lock().await;
        locks
            .entry(track_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Persists the current session to encrypted storage.
    pub async fn persist_session(&self) -> AppResult<()> {
        self.client()
            .await?
            .save_session()
            .await
            .map_err(|e| AppError::Telegram(e.to_string()))
    }

    /// Rebuild the ferogram client from current DB proxy settings.
    /// Clears in-flight auth tokens (they are invalid across reconnect).
    pub async fn rebuild_client(&self) -> AppResult<()> {
        let _guard = self.reconnect_lock.lock().await;

        let settings = proxy_settings::load(&self.db)?;
        let want_proxy = settings.for_connect();

        let (client, shutdown) =
            telegram::client::build(&self.config, &self.data_dir, want_proxy).await?;

        {
            let mut slot = self.telegram.write().await;
            if let Some(old) = slot.take() {
                old.shutdown.cancel();
            }
            *slot = Some(LiveTelegram { client, shutdown });
        }
        *self.proxy_active.lock().await = want_proxy.is_some();
        *self.proxy_last_error.lock().await = None;

        let mut pending = self.pending.lock().await;
        *pending = Default::default();

        Ok(())
    }

    /// Persist proxy settings and rebuild the client. On rebuild failure the
    /// previous settings are restored so SQLite stays aligned with the live
    /// client / `proxy_active` flag.
    pub async fn apply_proxy_settings(&self, settings: &ProxySettings) -> AppResult<()> {
        let previous = proxy_settings::load(&self.db)?;
        proxy_settings::save(&self.db, settings)?;
        match self.rebuild_client().await {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(rollback_err) = proxy_settings::save(&self.db, &previous) {
                    tracing::warn!(
                        "proxy rebuild failed and settings rollback also failed: {rollback_err}"
                    );
                }
                self.set_proxy_last_error(Some(err.to_string())).await;
                Err(err)
            }
        }
    }
}
