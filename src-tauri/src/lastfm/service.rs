use crate::time::now_ms;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::{Mutex, Notify};

#[cfg(not(target_os = "android"))]
use keyring as credential_store;
#[cfg(target_os = "android")]
use keyring_core as credential_store;

use crate::config::Config;
use crate::db::{
    Db, LastFmQueueInsert, SETTING_LASTFM_ACCOUNT_KEY, SETTING_LASTFM_ENABLED,
    SETTING_LASTFM_LAST_SCROBBLE_AT_MS, SETTING_LASTFM_NEEDS_REAUTH, SETTING_LASTFM_USERNAME,
};
use crate::error::{AppError, AppResult};
use crate::lastfm::api::{LastFmClient, LastFmError};
use crate::lastfm::models::{
    LastFmAuthState, LastFmSafeIssue, LastFmScrobble, LastFmStatus, PlaybackSnapshot,
};
use crate::state::AppState;

mod auth;
use auth::mark_runtime_needs_reauth;
mod playback;
pub(crate) use playback::qualification_threshold_ms;
mod queue;

/// Side effects supplied by the application boundary; services can run without Tauri.
pub trait LastFmHost: Send + Sync {
    fn publish(&self, status: LastFmStatus);
    fn open_url(&self, url: &str) -> AppResult<()>;
    fn invalidate_accounting(&self);
}

impl LastFmHost for AppHandle {
    fn publish(&self, status: LastFmStatus) {
        self.state::<AppState>().audio.lastfm_enabled.store(
            status.enabled && status.state == LastFmAuthState::Connected,
            std::sync::atomic::Ordering::Release,
        );
        let _ = self.emit("lastfm:status_changed", status);
    }
    fn open_url(&self, url: &str) -> AppResult<()> {
        self.opener()
            .open_url(url, None::<&str>)
            .map_err(|_| AppError::msg("The Last.fm page could not be opened."))
    }
    fn invalidate_accounting(&self) {
        self.state::<AppState>()
            .audio
            .lastfm_epoch
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
}

fn keyring_service() -> String {
    keyring_service_for(crate::session::credential_namespace())
}
fn keyring_service_for(identifier: &str) -> String {
    format!("{identifier}.lastfm")
}
const AUTH_TOKEN_TTL_MS: i64 = 60 * 60 * 1000;

trait SessionStore: Send + Sync {
    fn get(&self, username: &str) -> AppResult<Option<String>>;
    fn set(&self, username: &str, key: &str) -> AppResult<()>;
    fn delete(&self, username: &str) -> AppResult<()>;
}

struct OsSessionStore;

impl SessionStore for OsSessionStore {
    fn get(&self, username: &str) -> AppResult<Option<String>> {
        let entry = credential_store::Entry::new(&keyring_service(), username)
            .map_err(|_| AppError::msg("Last.fm keychain is unavailable."))?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(credential_store::Error::NoEntry)
                if crate::session::credential_namespace() != "com.soundgrammy.app" =>
            {
                match credential_store::Entry::new("com.soundgrammy.app.lastfm", username)
                    .and_then(|e| e.get_password())
                {
                    Ok(key) => {
                        entry
                            .set_password(&key)
                            .map_err(|_| AppError::msg("Last.fm credential migration failed."))?;
                        Ok(Some(key))
                    }
                    Err(credential_store::Error::NoEntry) => Ok(None),
                    Err(_) => Err(AppError::msg("Last.fm keychain could not be read.")),
                }
            }
            Err(credential_store::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::msg("Last.fm keychain could not be read.")),
        }
    }

    fn set(&self, username: &str, key: &str) -> AppResult<()> {
        credential_store::Entry::new(&keyring_service(), username)
            .and_then(|entry| entry.set_password(key))
            .map_err(|_| AppError::msg("Last.fm session could not be saved to the keychain."))
    }

    fn delete(&self, username: &str) -> AppResult<()> {
        let entry = credential_store::Entry::new(&keyring_service(), username)
            .map_err(|_| AppError::msg("Last.fm keychain is unavailable."))?;
        match entry.delete_credential() {
            Ok(()) | Err(credential_store::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppError::msg(
                "Last.fm session could not be removed from the keychain.",
            )),
        }
    }
}

#[derive(Clone)]
struct PendingToken {
    token: String,
    requested_at_ms: i64,
}

struct RuntimeState {
    generation: u64,
    auth_state: LastFmAuthState,
    username: Option<String>,
    account_key: Option<String>,
    session_key: Option<String>,
    enabled: bool,
    pending: Option<PendingToken>,
    last_error: Option<LastFmSafeIssue>,
    last_metadata_warning: Option<LastFmSafeIssue>,
    active_attempts: HashMap<String, PlaybackSnapshot>,
}

pub struct LastFmService {
    client: Option<Arc<LastFmClient>>,
    session_store: Arc<dyn SessionStore>,
    runtime: Mutex<RuntimeState>,
    worker_lock: Mutex<()>,
    notify: Notify,
    now_playing: Mutex<Option<(u64, String, String, PlaybackSnapshot)>>,
}

impl LastFmService {
    pub fn new(config: &Config, db: &Db) -> Self {
        Self::new_with_store(config, db, Arc::new(OsSessionStore))
    }

    fn new_with_store(config: &Config, db: &Db, session_store: Arc<dyn SessionStore>) -> Self {
        let client = config
            .lastfm
            .clone()
            .and_then(|credentials| LastFmClient::production(credentials).ok())
            .map(Arc::new);
        let username = db.get_setting(SETTING_LASTFM_USERNAME).ok().flatten();
        let account_key = db.get_setting(SETTING_LASTFM_ACCOUNT_KEY).ok().flatten();
        let enabled = db
            .get_setting(SETTING_LASTFM_ENABLED)
            .ok()
            .flatten()
            .as_deref()
            == Some("true");
        let needs_reauth = db
            .get_setting(SETTING_LASTFM_NEEDS_REAUTH)
            .ok()
            .flatten()
            .as_deref()
            == Some("true");
        let session_key = username
            .as_deref()
            .and_then(|name| session_store.get(name).ok().flatten());
        let auth_state = if client.is_none() {
            LastFmAuthState::UnavailableInBuild
        } else if username.is_some() && session_key.is_some() && needs_reauth {
            LastFmAuthState::NeedsReauthentication
        } else if username.is_some() && session_key.is_some() {
            LastFmAuthState::Connected
        } else if username.is_some() {
            LastFmAuthState::NeedsReauthentication
        } else {
            LastFmAuthState::Disconnected
        };
        Self {
            client,
            session_store,
            runtime: Mutex::new(RuntimeState {
                generation: 0,
                auth_state,
                username,
                account_key,
                session_key,
                enabled,
                pending: None,
                last_error: None,
                last_metadata_warning: None,
                active_attempts: HashMap::new(),
            }),
            worker_lock: Mutex::new(()),
            notify: Notify::new(),
            now_playing: Mutex::new(None),
        }
    }

    pub async fn status(&self, db: &Db) -> AppResult<LastFmStatus> {
        let runtime = self.runtime.lock().await;
        let pending_count = match runtime.account_key.as_deref() {
            Some(key) => db.lastfm_pending_count(key)?,
            None => 0,
        };
        let last_scrobble_at_ms = db
            .get_setting(SETTING_LASTFM_LAST_SCROBBLE_AT_MS)?
            .and_then(|value| value.parse().ok());
        Ok(LastFmStatus {
            state: runtime.auth_state,
            username: runtime.username.clone(),
            enabled: runtime.enabled,
            pending_count,
            retained_queues: db.lastfm_queue_summaries()?,
            last_scrobble_at_ms,
            last_error: runtime.last_error.clone(),
            last_metadata_warning: runtime.last_metadata_warning.clone(),
        })
    }

    async fn emit_status(&self, db: &Db, app: &dyn LastFmHost) {
        if let Ok(status) = self.status(db).await {
            app.publish(status);
        }
    }

    fn client(&self) -> AppResult<Arc<LastFmClient>> {
        self.client
            .clone()
            .ok_or_else(|| AppError::msg("Last.fm is unavailable in this build."))
    }

    pub async fn set_enabled(
        &self,
        db: &Db,
        app: &dyn LastFmHost,
        enabled: bool,
    ) -> AppResult<LastFmStatus> {
        {
            let mut runtime = self.runtime.lock().await;
            if runtime.username.is_none() || runtime.session_key.is_none() {
                return Err(AppError::msg("Connect Last.fm before enabling scrobbling."));
            }
            db.set_setting(
                SETTING_LASTFM_ENABLED,
                if enabled { "true" } else { "false" },
            )?;
            runtime.enabled = enabled;
            runtime.active_attempts.clear();
            runtime.generation += 1;
            app.invalidate_accounting();
        }
        if enabled {
            self.wake();
        }
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub async fn disconnect(
        &self,
        db: &Db,
        app: &dyn LastFmHost,
        pending_action: Option<&str>,
    ) -> AppResult<LastFmStatus> {
        {
            let mut runtime = self.runtime.lock().await;
            let delete_queue = if let Some(key) = runtime.account_key.as_deref() {
                if db.lastfm_pending_count(key)? > 0 {
                    match pending_action {
                        Some("retain") => false,
                        Some("delete") => true,
                        _ => {
                            return Err(AppError::msg(
                                "Choose whether to retain or delete pending Last.fm scrobbles.",
                            ))
                        }
                    }
                } else {
                    false
                }
            } else {
                false
            };
            if let Some(username) = runtime.username.as_deref() {
                self.session_store.delete(username)?;
            }
            let result = db.update_settings_and_queue(
                &[
                    (SETTING_LASTFM_USERNAME, None),
                    (SETTING_LASTFM_ACCOUNT_KEY, None),
                    (SETTING_LASTFM_LAST_SCROBBLE_AT_MS, None),
                    (SETTING_LASTFM_NEEDS_REAUTH, None),
                    (SETTING_LASTFM_ENABLED, Some("false")),
                ],
                if delete_queue {
                    runtime.account_key.as_deref()
                } else {
                    None
                },
            );
            if let Err(error) = result {
                if let (Some(username), Some(key)) = (&runtime.username, &runtime.session_key) {
                    if self.session_store.set(username, key).is_err() {
                        runtime.auth_state = LastFmAuthState::NeedsReauthentication;
                        runtime.enabled = false;
                        runtime.session_key = None;
                        runtime.active_attempts.clear();
                        runtime.last_error = Some(issue(
                            "storage",
                            None,
                            "Last.fm credentials require recovery. Reconnect the account.",
                        ));
                        app.invalidate_accounting();
                        drop(runtime);
                        self.emit_status(db, app).await;
                        return Err(AppError::CredentialRecovery);
                    }
                }
                return Err(error);
            }
            runtime.generation += 1;
            runtime.auth_state = if self.client.is_some() {
                LastFmAuthState::Disconnected
            } else {
                LastFmAuthState::UnavailableInBuild
            };
            runtime.username = None;
            runtime.account_key = None;
            runtime.session_key = None;
            runtime.enabled = false;
            runtime.pending = None;
            runtime.last_error = None;
            runtime.last_metadata_warning = None;
            runtime.active_attempts.clear();
            app.invalidate_accounting();
        }
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub async fn open_profile(&self, app: &dyn LastFmHost) -> AppResult<()> {
        let username = self
            .runtime
            .lock()
            .await
            .username
            .clone()
            .ok_or_else(|| AppError::msg("Last.fm is not connected."))?;
        let mut url = url::Url::parse("https://www.last.fm/user/")
            .map_err(|_| AppError::msg("Last.fm profile URL is invalid."))?;
        url.path_segments_mut()
            .map_err(|_| AppError::msg("Last.fm profile URL is invalid."))?
            .push(&username);
        app.open_url(url.as_str())
    }

    pub fn wake(&self) {
        self.notify.notify_one();
    }
}

fn issue(kind: &str, code: Option<i64>, message: &str) -> LastFmSafeIssue {
    LastFmSafeIssue {
        kind: kind.into(),
        code,
        message: message.into(),
        at_ms: now_ms(),
    }
}

fn api_code(error: &LastFmError) -> Option<i64> {
    match error {
        LastFmError::Api(code) => Some(*code),
        _ => None,
    }
}

fn account_key(username: &str) -> String {
    username.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct MemorySessionStore {
        fail_set: std::sync::atomic::AtomicBool,
        values: StdMutex<HashMap<String, String>>,
        fail_reads: bool,
    }

    impl SessionStore for MemorySessionStore {
        fn get(&self, username: &str) -> AppResult<Option<String>> {
            if self.fail_reads {
                return Err(AppError::msg("keyring read failed"));
            }
            Ok(self.values.lock().unwrap().get(username).cloned())
        }

        fn set(&self, username: &str, key: &str) -> AppResult<()> {
            if self.fail_set.load(std::sync::atomic::Ordering::Acquire) {
                return Err(AppError::msg("injected keyring write failure"));
            }
            self.values
                .lock()
                .unwrap()
                .insert(username.into(), key.into());
            Ok(())
        }

        fn delete(&self, username: &str) -> AppResult<()> {
            self.values.lock().unwrap().remove(username);
            Ok(())
        }
    }

    #[test]
    fn session_store_round_trips_and_deletes_without_sqlite_secrets() {
        let store = MemorySessionStore::default();
        assert_eq!(store.get("alice").unwrap(), None);
        store.set("alice", "session-key").unwrap();
        assert_eq!(store.get("alice").unwrap().as_deref(), Some("session-key"));
        store.delete("alice").unwrap();
        assert_eq!(store.get("alice").unwrap(), None);
    }
    #[derive(Default)]
    struct TestHost;
    impl LastFmHost for TestHost {
        fn publish(&self, _: LastFmStatus) {}
        fn open_url(&self, _: &str) -> AppResult<()> {
            Ok(())
        }
        fn invalidate_accounting(&self) {}
    }

    #[tokio::test]
    async fn completing_without_pending_auth_never_holds_the_status_lock() {
        let db = crate::db::test_db().unwrap();
        let service = LastFmService::new_with_store(
            &Config {
                api_id: 1,
                api_hash: "test".into(),
                lastfm: Some(crate::config::LastFmCredentials {
                    api_key: "test".into(),
                    api_secret: "test".into(),
                }),
            },
            &db,
            Arc::new(MemorySessionStore::default()),
        );
        for _ in 0..2 {
            tokio::time::timeout(
                Duration::from_millis(200),
                service.complete_auth(&db, &TestHost),
            )
            .await
            .expect("completion deadlocked")
            .unwrap();
            service.cancel_auth(&db, &TestHost).await.unwrap();
        }
        tokio::time::timeout(Duration::from_millis(200), service.status(&db))
            .await
            .expect("status deadlocked")
            .unwrap();
    }

    struct DelayedTransport {
        delayed_method: &'static str,
        started: Arc<Notify>,
        release: Arc<Notify>,
        error: bool,
    }
    #[async_trait::async_trait]
    impl crate::lastfm::api::LastFmTransport for DelayedTransport {
        async fn post(
            &self,
            params: &std::collections::BTreeMap<String, String>,
        ) -> Result<serde_json::Value, LastFmError> {
            let method = params.get("method").unwrap().as_str();
            if method == self.delayed_method {
                self.started.notify_one();
                self.release.notified().await;
                if self.error {
                    return Err(LastFmError::Api(9));
                }
            }
            Ok(match method {
                "auth.getToken" => serde_json::json!({"token":"token"}),
                "auth.getSession" => serde_json::json!({"session":{"name":"Bob","key":"bob-key"}}),
                "user.getInfo" => serde_json::json!({"user":{"name":"Bob"}}),
                "track.updateNowPlaying" => {
                    serde_json::json!({"nowplaying":{"ignoredMessage":{"code":"0"}}})
                }
                _ => serde_json::json!({"scrobbles":{"scrobble":{"ignoredMessage":{"code":"0"}}}}),
            })
        }
    }
    fn delayed_service(
        db: &Db,
        method: &'static str,
        error: bool,
    ) -> (
        Arc<LastFmService>,
        Arc<MemorySessionStore>,
        Arc<Notify>,
        Arc<Notify>,
    ) {
        let store = Arc::new(MemorySessionStore::default());
        let mut service = LastFmService::new_with_store(
            &Config {
                api_id: 1,
                api_hash: "test".into(),
                lastfm: None,
            },
            db,
            store.clone(),
        );
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        service.client = Some(Arc::new(LastFmClient::with_transport(
            crate::config::LastFmCredentials {
                api_key: "test".into(),
                api_secret: "test".into(),
            },
            Box::new(DelayedTransport {
                delayed_method: method,
                started: started.clone(),
                release: release.clone(),
                error,
            }),
        )));
        (Arc::new(service), store, started, release)
    }
    #[tokio::test]
    async fn delayed_token_and_session_responses_cannot_resurrect_cancelled_auth() {
        for method in ["auth.getToken", "auth.getSession"] {
            let db = Arc::new(crate::db::test_db().unwrap());
            let (service, store, started, release) = delayed_service(&db, method, false);
            if method == "auth.getSession" {
                service.start_auth(&db, &TestHost).await.unwrap();
            }
            let task = {
                let service = service.clone();
                let db = db.clone();
                tokio::spawn(async move {
                    if method == "auth.getToken" {
                        service.start_auth(&db, &TestHost).await
                    } else {
                        service.complete_auth(&db, &TestHost).await
                    }
                })
            };
            started.notified().await;
            service.cancel_auth(&db, &TestHost).await.unwrap();
            release.notify_one();
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                service.status(&db).await.unwrap().state,
                LastFmAuthState::Disconnected
            );
            assert!(store.get("Bob").unwrap().is_none());
            assert!(db
                .get_setting(SETTING_LASTFM_ACCOUNT_KEY)
                .unwrap()
                .is_none());
        }
    }
    #[tokio::test]
    async fn failed_persistence_does_not_toggle_runtime_or_destroy_credentials() {
        let db = crate::db::test_db().unwrap();
        let (service, store, _, _) = delayed_service(&db, "unused", false);
        service.start_auth(&db, &TestHost).await.unwrap();
        service.complete_auth(&db, &TestHost).await.unwrap();
        db.conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_settings BEFORE UPDATE ON app_settings
            BEGIN SELECT RAISE(ABORT,'injected write failure'); END;",
            )
            .unwrap();
        assert!(service.set_enabled(&db, &TestHost, true).await.is_err());
        assert!(!service.status(&db).await.unwrap().enabled);
        assert!(service
            .disconnect(&db, &TestHost, Some("retain"))
            .await
            .is_err());
        assert_eq!(store.get("Bob").unwrap().as_deref(), Some("bob-key"));
        assert_eq!(
            service.status(&db).await.unwrap().state,
            LastFmAuthState::Connected
        );
    }
    #[tokio::test]
    async fn slow_now_playing_does_not_delay_durable_qualification_or_change_start_snapshot() {
        let state = crate::cache::tests::test_state();
        state.db.save_profile(1, "User", None, None, None).unwrap();
        let track_id = crate::cache::tests::insert_track(&state, 1, "123");
        let db = state.db;
        let (service, _, started, release) = delayed_service(&db, "track.updateNowPlaying", false);
        service.start_auth(&db, &TestHost).await.unwrap();
        service.complete_auth(&db, &TestHost).await.unwrap();
        service.set_enabled(&db, &TestHost, true).await.unwrap();
        service
            .register_started(
                &db,
                &TestHost,
                crate::lastfm::models::PlaybackStart {
                    attempt_id: "test-attempt".into(),
                    track_id,
                    started_at_utc: 12345,
                    duration_seconds: Some(120),
                },
            )
            .await
            .unwrap();
        let task = {
            let service = service.clone();
            let db = db.clone();
            tokio::spawn(async move { service.deliver_now_playing(&db, &TestHost).await })
        };
        started.notified().await;
        tokio::time::timeout(
            Duration::from_millis(200),
            service.attempt_qualified(&db, &TestHost, "test-attempt", 60000),
        )
        .await
        .expect("HTTP blocked qualification")
        .unwrap();
        service
            .attempt_qualified(&db, &TestHost, "test-attempt", 60000)
            .await
            .unwrap();
        let account = service.runtime.lock().await.account_key.clone().unwrap();
        let rows = db.due_lastfm_scrobbles(&account, i64::MAX, 50).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].started_at_utc, 12345);
        assert_eq!(rows[0].duration_seconds, Some(120));
        release.notify_one();
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn late_queue_auth_error_cannot_invalidate_a_replacement_connection() {
        let db = crate::db::test_db().unwrap();
        let (service, store, started, release) = delayed_service(&db, "track.scrobble", true);
        service.start_auth(&db, &TestHost).await.unwrap();
        service.complete_auth(&db, &TestHost).await.unwrap();
        service.set_enabled(&db, &TestHost, true).await.unwrap();
        let original = service.runtime.lock().await.account_key.clone().unwrap();
        db.enqueue_lastfm_scrobble(&LastFmQueueInsert {
            attempt_id: "old-attempt".into(),
            username: "Bob".into(),
            account_key: original.clone(),
            track_id: None,
            artist: "Artist".into(),
            track_title: "Track".into(),
            duration_seconds: Some(120),
            started_at_utc: 12345,
            created_at_ms: 12345,
        })
        .unwrap();
        let task = {
            let service = service.clone();
            let db = db.clone();
            tokio::spawn(async move { service.flush_once(&db, &TestHost).await })
        };
        started.notified().await;
        service
            .disconnect(&db, &TestHost, Some("retain"))
            .await
            .unwrap();
        service.start_auth(&db, &TestHost).await.unwrap();
        service.complete_auth(&db, &TestHost).await.unwrap();
        release.notify_one();
        task.await.unwrap().unwrap();
        assert_eq!(
            service.status(&db).await.unwrap().state,
            LastFmAuthState::Connected
        );
        assert_eq!(store.get("Bob").unwrap().as_deref(), Some("bob-key"));
        assert_eq!(db.lastfm_pending_count(&original).unwrap(), 1);
        assert_ne!(
            db.get_setting(SETTING_LASTFM_NEEDS_REAUTH)
                .unwrap()
                .as_deref(),
            Some("true")
        );
    }
    #[tokio::test]
    async fn late_success_receipt_settles_original_rows_without_updating_new_account_status() {
        let db = crate::db::test_db().unwrap();
        let (service, _, started, release) = delayed_service(&db, "track.scrobble", false);
        service.start_auth(&db, &TestHost).await.unwrap();
        service.complete_auth(&db, &TestHost).await.unwrap();
        service.set_enabled(&db, &TestHost, true).await.unwrap();
        let original = service.runtime.lock().await.account_key.clone().unwrap();
        db.enqueue_lastfm_scrobble(&LastFmQueueInsert {
            attempt_id: "old-attempt".into(),
            username: "Bob".into(),
            account_key: original.clone(),
            track_id: None,
            artist: "Artist".into(),
            track_title: "Track".into(),
            duration_seconds: Some(120),
            started_at_utc: 12345,
            created_at_ms: 12345,
        })
        .unwrap();
        let task = {
            let service = service.clone();
            let db = db.clone();
            tokio::spawn(async move { service.flush_once(&db, &TestHost).await })
        };
        started.notified().await;
        service
            .disconnect(&db, &TestHost, Some("retain"))
            .await
            .unwrap();
        release.notify_one();
        task.await.unwrap().unwrap();
        assert_eq!(db.lastfm_pending_count(&original).unwrap(), 0);
        assert_eq!(
            service.status(&db).await.unwrap().state,
            LastFmAuthState::Disconnected
        );
        assert!(db
            .get_setting(SETTING_LASTFM_LAST_SCROBBLE_AT_MS)
            .unwrap()
            .is_none());
    }

    #[test]
    fn development_and_release_configs_select_distinct_credentials() {
        let release: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        let desktop: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.dev.conf.json")).unwrap();
        let android: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.android.dev.conf.json")).unwrap();
        let production = release["identifier"].as_str().unwrap();
        assert_eq!(
            keyring_service_for(production),
            "com.soundgrammy.app.lastfm"
        );
        let desktop_id = desktop["identifier"].as_str().unwrap();
        let suffix = android["bundle"]["android"]["debugApplicationIdSuffix"]
            .as_str()
            .unwrap();
        let android_id = crate::session::app_namespace(production, Some(suffix));
        for identifier in [desktop_id, android_id.as_str()] {
            assert_ne!(identifier, production);
            assert_ne!(
                keyring_service_for(identifier),
                keyring_service_for(production)
            );
        }
        assert_eq!(crate::session::app_namespace(production, None), production);
    }
    #[tokio::test]
    async fn credential_and_db_commit_failures_leave_auth_retryable_and_never_half_connect() {
        let db = crate::db::test_db().unwrap();
        let (service, store, _, _) = delayed_service(&db, "unused", false);
        service.start_auth(&db, &TestHost).await.unwrap();
        store
            .fail_set
            .store(true, std::sync::atomic::Ordering::Release);
        assert!(service.complete_auth(&db, &TestHost).await.is_err());
        assert_eq!(
            service.status(&db).await.unwrap().state,
            LastFmAuthState::WaitingForBrowserApproval
        );
        assert!(db.get_setting(SETTING_LASTFM_USERNAME).unwrap().is_none());
        store
            .fail_set
            .store(false, std::sync::atomic::Ordering::Release);
        db.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_commit BEFORE INSERT ON app_settings BEGIN SELECT RAISE(ABORT,'injected DB failure'); END;").unwrap();
        assert!(service.complete_auth(&db, &TestHost).await.is_err());
        assert!(store.get("Bob").unwrap().is_none());
        assert!(db.get_setting(SETTING_LASTFM_USERNAME).unwrap().is_none());
        assert_eq!(
            service.status(&db).await.unwrap().state,
            LastFmAuthState::WaitingForBrowserApproval
        );
        db.conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_commit")
            .unwrap();
        service.complete_auth(&db, &TestHost).await.unwrap();
        assert_eq!(
            service.status(&db).await.unwrap().state,
            LastFmAuthState::Connected
        );
    }

    #[tokio::test]
    async fn failed_compensation_exposes_reauthentication_instead_of_a_mixed_connected_state() {
        let db = crate::db::test_db().unwrap();
        let (service, store, _, _) = delayed_service(&db, "unused", false);
        service.start_auth(&db, &TestHost).await.unwrap();
        service.complete_auth(&db, &TestHost).await.unwrap();
        service.set_enabled(&db, &TestHost, true).await.unwrap();
        db.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_disconnect BEFORE UPDATE ON app_settings BEGIN SELECT RAISE(ABORT,'injected DB failure'); END;").unwrap();
        store
            .fail_set
            .store(true, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            service.disconnect(&db, &TestHost, None).await,
            Err(AppError::CredentialRecovery)
        ));
        let status = service.status(&db).await.unwrap();
        assert_eq!(status.state, LastFmAuthState::NeedsReauthentication);
        assert!(!status.enabled);
        assert!(store.get("Bob").unwrap().is_none());
    }
}
