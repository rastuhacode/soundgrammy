//! Shared application state managed by Tauri.
//!
//! A ferogram [`Client`] may be absent at startup when Telegram is unreachable
//! (blocked network / misconfigured proxy). The login UI can still load so the
//! user can enable an MTProto proxy and reconnect.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicI64, AtomicU64, Ordering},
    Arc, Weak,
};

use ferogram::{Client, LoginToken, PasswordToken, ShutdownToken};
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::lastfm::LastFmService;
use crate::proxy_settings::{self, ProxySettings};
use crate::streaming::StreamingManager;
use crate::telegram;
use crate::telegram::media_requests::MediaRequestCoordinator;

/// Transient authentication state kept between login command calls.
#[derive(Default, Clone)]
pub enum PendingAuth {
    #[default]
    Idle,
    Phone(LoginToken),
    Password(Box<PasswordToken>),
    Qr,
}

struct LiveTelegram {
    client: Client,
    shutdown: ShutdownToken,
    epoch: u64,
}

pub struct AppState {
    pub audio: crate::audio::NativeAudioService,
    pub config: Config,
    pub db: Db,
    telegram: RwLock<Option<LiveTelegram>>,
    /// Serializes client rebuilds (proxy apply / disable).
    reconnect_lock: Mutex<()>,
    pub auth_operations: Mutex<()>,
    pub client_generation: AtomicU64,
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
    pub download_locks: Mutex<HashMap<String, Weak<Mutex<()>>>>,
    /// Per-track locks deduplicate profile requests; the slot keeps CPU-heavy
    /// decoding to one track at a time.
    pub bounce_analysis_locks: Mutex<HashMap<i64, Weak<Mutex<()>>>>,
    pub bounce_analysis_slot: Mutex<()>,
    pub bounce_requested_track: AtomicI64,
    /// Active progressive audio downloads, shared by playback and explicit saves.
    pub streaming: StreamingManager,
    pub cache_lifecycle: Mutex<()>,
    pub cache_leases: crate::cache::LeaseRegistry,
    pub cache_budget: crate::cache::BudgetRegistry,
    /// Aggregate Telegram media pacing shared by audio, artwork, and bulk downloads.
    pub media_requests: MediaRequestCoordinator,
    pub lastfm: LastFmService,
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
        let telegram = client.map(|(client, shutdown)| LiveTelegram {
            client,
            shutdown,
            epoch: 0,
        });
        let lastfm = LastFmService::new(&config, &db);
        let audio = crate::audio::NativeAudioService::default();
        audio.local_listens_enabled.store(
            db.get_setting(crate::db::SETTING_LISTEN_STATS_ENABLED)
                .ok()
                .flatten()
                .as_deref()
                != Some("false"),
            Ordering::Release,
        );
        Self {
            audio,
            config,
            db,
            telegram: RwLock::new(telegram),
            reconnect_lock: Mutex::new(()),
            auth_operations: Mutex::new(()),
            client_generation: AtomicU64::new(0),
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
            cache_lifecycle: Default::default(),
            cache_leases: Default::default(),
            cache_budget: Default::default(),
            media_requests: Default::default(),
            lastfm,
        }
    }

    pub async fn auth_client(&self) -> AppResult<(Client, u64)> {
        let live = self.telegram.read().await;
        match live.as_ref() {
            Some(live) => Ok((live.client.clone(), live.epoch)),
            None => Err(AppError::msg("Telegram is offline")),
        }
    }
    pub async fn guard_client(&self, epoch: u64) -> AppResult<tokio::sync::MutexGuard<'_, ()>> {
        let guard = self.reconnect_lock.lock().await;
        if self.client_generation.load(Ordering::Acquire) != epoch {
            return Err(AppError::NotAuthorized);
        }
        Ok(guard)
    }
    pub(crate) async fn guard_lifecycle(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.reconnect_lock.lock().await
    }

    /// Cheap clone of the live ferogram client handle.
    pub async fn client(&self) -> AppResult<Client> {
        if let Some(client) = self.current_client().await {
            return Ok(client);
        }

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

    async fn current_client(&self) -> Option<Client> {
        self.telegram
            .read()
            .await
            .as_ref()
            .map(|live| live.client.clone())
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
        weak_key_lock(&self.download_locks, key.to_string()).await
    }
    pub async fn bounce_lock_for(&self, track_id: i64) -> Arc<Mutex<()>> {
        weak_key_lock(&self.bounce_analysis_locks, track_id).await
    }

    /// Connect on demand without replacing a client another caller just built.
    /// This keeps app startup local-only while preserving first-time login.
    pub async fn ensure_client(&self) -> AppResult<Client> {
        if let Some(client) = self.current_client().await {
            return Ok(client);
        }

        let _guard = self.reconnect_lock.lock().await;
        if let Some(client) = self.current_client().await {
            return Ok(client);
        }

        self.rebuild_client_locked().await?;
        self.client().await
    }

    pub(crate) async fn disconnect_client_locked(&self) {
        self.client_generation.fetch_add(1, Ordering::AcqRel);
        let old = self.telegram.write().await.take();
        if let Some(old) = old {
            old.client.disconnect();
            old.shutdown.cancel();
        }
        *self.proxy_active.lock().await = false;

        let mut pending = self.pending.lock().await;
        *pending = Default::default();
    }

    async fn rebuild_client_locked(&self) -> AppResult<()> {
        let settings = proxy_settings::load(&self.db)?;
        let want_proxy = settings.for_connect();

        // Telegram normally permits only one main session per authorization
        // key. Stop the old connection before loading that key into a new one.
        self.disconnect_client_locked().await;
        tokio::task::yield_now().await;

        let (client, shutdown) =
            match telegram::client::build(&self.config, &self.data_dir, want_proxy).await {
                Ok(pair) => pair,
                Err(err) => {
                    self.set_proxy_last_error(Some(err.to_string())).await;
                    return Err(err);
                }
            };

        *self.telegram.write().await = Some(LiveTelegram {
            client,
            shutdown,
            epoch: self.client_generation.load(Ordering::Acquire),
        });
        *self.proxy_active.lock().await = want_proxy.is_some();
        *self.proxy_last_error.lock().await = None;

        Ok(())
    }

    pub async fn proxy_view(&self) -> AppResult<proxy_settings::ProxySettingsView> {
        let _guard = self.reconnect_lock.lock().await;
        Ok(proxy_settings::ProxySettingsView::from_parts(
            proxy_settings::load(&self.db)?,
            self.proxy_active().await,
            self.proxy_last_error().await,
            self.is_telegram_online().await,
        ))
    }

    /// Persist proxy settings and rebuild the client. On rebuild failure the
    /// previous settings are restored. The client remains offline so the
    /// reconnect loop can retry with those restored settings.
    pub async fn apply_proxy_settings(&self, settings: &ProxySettings) -> AppResult<()> {
        self.apply_proxy_settings_with(settings, || self.rebuild_client_locked())
            .await
    }

    async fn apply_proxy_settings_with<F>(
        &self,
        settings: &ProxySettings,
        rebuild: impl FnOnce() -> F,
    ) -> AppResult<()>
    where
        F: std::future::Future<Output = AppResult<()>>,
    {
        let _guard = self.reconnect_lock.lock().await;
        let previous = proxy_settings::load(&self.db)?;
        proxy_settings::save(&self.db, settings)?;
        match rebuild().await {
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

async fn weak_key_lock<K: std::hash::Hash + Eq>(
    registry: &Mutex<HashMap<K, Weak<Mutex<()>>>>,
    key: K,
) -> Arc<Mutex<()>> {
    let mut locks = registry.lock().await;
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    lock
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn inactive_key_locks_are_pruned_without_splitting_active_ownership() {
        let state = crate::cache::tests::test_state();
        let active = state.lock_for("active").await;
        for key in 0..1000 {
            drop(state.lock_for(&key.to_string()).await);
        }
        assert!(state.download_locks.lock().await.len() <= 2);
        assert!(Arc::ptr_eq(&active, &state.lock_for("active").await));
        for key in 0..1000 {
            drop(state.bounce_lock_for(key).await);
        }
        assert!(state.bounce_analysis_locks.lock().await.len() <= 1);
    }
    #[tokio::test]
    async fn concurrent_proxy_failure_cannot_rollback_the_next_request() {
        let state = Arc::new(crate::cache::tests::test_state());
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let a = ProxySettings {
            server: "A".into(),
            ..Default::default()
        };
        let b = ProxySettings {
            server: "B".into(),
            ..Default::default()
        };
        let first = {
            let state = state.clone();
            let started = started.clone();
            let release = release.clone();
            tokio::spawn(async move {
                state
                    .apply_proxy_settings_with(&a, || async {
                        started.notify_one();
                        release.notified().await;
                        Err(AppError::msg("injected connect failure"))
                    })
                    .await
            })
        };
        started.notified().await;
        let second = {
            let state = state.clone();
            tokio::spawn(async move {
                state
                    .apply_proxy_settings_with(&b, || async { Ok(()) })
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert_eq!(proxy_settings::load(&state.db).unwrap().server, "A");
        release.notify_one();
        assert!(first.await.unwrap().is_err());
        second.await.unwrap().unwrap();
        assert_eq!(proxy_settings::load(&state.db).unwrap().server, "B");
    }
}
