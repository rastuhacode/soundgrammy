use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::{Mutex, Notify};

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
mod playback;
mod queue;

const KEYRING_SERVICE: &str = "com.soundgrammy.app.lastfm";
const AUTH_TOKEN_TTL_MS: i64 = 60 * 60 * 1000;

trait SessionStore: Send + Sync {
    fn get(&self, username: &str) -> AppResult<Option<String>>;
    fn set(&self, username: &str, key: &str) -> AppResult<()>;
    fn delete(&self, username: &str) -> AppResult<()>;
}

struct OsSessionStore;

impl SessionStore for OsSessionStore {
    fn get(&self, username: &str) -> AppResult<Option<String>> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, username)
            .map_err(|_| AppError::msg("Last.fm keychain is unavailable."))?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::msg("Last.fm keychain could not be read.")),
        }
    }

    fn set(&self, username: &str, key: &str) -> AppResult<()> {
        keyring::Entry::new(KEYRING_SERVICE, username)
            .and_then(|entry| entry.set_password(key))
            .map_err(|_| AppError::msg("Last.fm session could not be saved to the keychain."))
    }

    fn delete(&self, username: &str) -> AppResult<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, username)
            .map_err(|_| AppError::msg("Last.fm keychain is unavailable."))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
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

    async fn emit_status(&self, db: &Db, app: &AppHandle) {
        if let Ok(status) = self.status(db).await {
            let _ = app.emit("lastfm:status_changed", status);
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
        app: &AppHandle,
        enabled: bool,
    ) -> AppResult<LastFmStatus> {
        {
            let mut runtime = self.runtime.lock().await;
            if runtime.username.is_none() || runtime.session_key.is_none() {
                return Err(AppError::msg("Connect Last.fm before enabling scrobbling."));
            }
            runtime.enabled = enabled;
            runtime.active_attempts.clear();
        }
        db.set_setting(
            SETTING_LASTFM_ENABLED,
            if enabled { "true" } else { "false" },
        )?;
        if enabled {
            self.wake();
        }
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub async fn disconnect(
        &self,
        db: &Db,
        app: &AppHandle,
        pending_action: Option<&str>,
    ) -> AppResult<LastFmStatus> {
        let (username, account_key) = {
            let runtime = self.runtime.lock().await;
            (runtime.username.clone(), runtime.account_key.clone())
        };
        if let Some(account_key) = account_key.as_deref() {
            let count = db.lastfm_pending_count(account_key)?;
            if count > 0 {
                match pending_action {
                    Some("retain") => {}
                    Some("delete") => db.delete_lastfm_queue_for(account_key)?,
                    _ => {
                        return Err(AppError::msg(
                            "Choose whether to retain or delete pending Last.fm scrobbles.",
                        ));
                    }
                }
            }
        }
        if let Some(username) = username.as_deref() {
            self.session_store.delete(username)?;
        }
        for key in [
            SETTING_LASTFM_USERNAME,
            SETTING_LASTFM_ACCOUNT_KEY,
            SETTING_LASTFM_LAST_SCROBBLE_AT_MS,
            SETTING_LASTFM_NEEDS_REAUTH,
        ] {
            db.delete_setting(key)?;
        }
        db.set_setting(SETTING_LASTFM_ENABLED, "false")?;
        {
            let mut runtime = self.runtime.lock().await;
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
        }
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub async fn open_profile(&self, app: &AppHandle) -> AppResult<()> {
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
        app.opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(|_| AppError::msg("The Last.fm profile could not be opened."))
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

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct MemorySessionStore {
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
}
