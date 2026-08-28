use super::*;

impl LastFmService {
    pub async fn start_auth(&self, db: &Db, app: &AppHandle) -> AppResult<LastFmStatus> {
        let client = self.client()?;
        {
            let mut runtime = self.runtime.lock().await;
            runtime.auth_state = LastFmAuthState::RequestingToken;
            runtime.pending = None;
            runtime.last_error = None;
        }
        self.emit_status(db, app).await;

        let token = match client.get_token().await {
            Ok(token) => token,
            Err(error) => {
                self.set_error(error, false).await;
                self.emit_status(db, app).await;
                return self.status(db).await;
            }
        };
        let url = client
            .authorization_url(&token)
            .map_err(|_| AppError::msg("Last.fm authorization could not be started."))?;
        {
            let mut runtime = self.runtime.lock().await;
            runtime.pending = Some(PendingToken {
                token,
                requested_at_ms: now_ms(),
            });
            runtime.auth_state = LastFmAuthState::WaitingForBrowserApproval;
        }
        self.emit_status(db, app).await;
        if app.opener().open_url(url, None::<&str>).is_err() {
            let mut runtime = self.runtime.lock().await;
            runtime.pending = None;
            runtime.auth_state = LastFmAuthState::Error;
            runtime.last_error = Some(issue(
                "authorization",
                None,
                "The Last.fm authorization page could not be opened.",
            ));
            drop(runtime);
            self.emit_status(db, app).await;
        }
        self.status(db).await
    }

    pub async fn complete_auth(&self, db: &Db, app: &AppHandle) -> AppResult<LastFmStatus> {
        let client = self.client()?;
        let pending = {
            let mut runtime = self.runtime.lock().await;
            let Some(pending) = runtime.pending.clone() else {
                return self.status(db).await;
            };
            if now_ms() - pending.requested_at_ms >= AUTH_TOKEN_TTL_MS {
                runtime.pending = None;
                runtime.auth_state = LastFmAuthState::Error;
                runtime.last_error = Some(issue(
                    "authorization",
                    Some(15),
                    "The Last.fm authorization request expired.",
                ));
                drop(runtime);
                self.emit_status(db, app).await;
                return self.status(db).await;
            }
            runtime.auth_state = LastFmAuthState::ExchangingSession;
            pending
        };
        self.emit_status(db, app).await;

        let session = match client.get_session(&pending.token).await {
            Ok(session) => session,
            Err(LastFmError::Api(14)) => {
                let mut runtime = self.runtime.lock().await;
                runtime.auth_state = LastFmAuthState::WaitingForBrowserApproval;
                drop(runtime);
                self.emit_status(db, app).await;
                return self.status(db).await;
            }
            Err(LastFmError::Api(15)) => {
                let mut runtime = self.runtime.lock().await;
                runtime.pending = None;
                runtime.auth_state = LastFmAuthState::Error;
                runtime.last_error = Some(issue(
                    "authorization",
                    Some(15),
                    "The Last.fm authorization request expired.",
                ));
                drop(runtime);
                self.emit_status(db, app).await;
                return self.status(db).await;
            }
            Err(error) => {
                self.set_error(error, false).await;
                self.emit_status(db, app).await;
                return self.status(db).await;
            }
        };

        let verified_username = client
            .verify_user(&session.key)
            .await
            .unwrap_or_else(|_| session.username.clone());
        self.session_store.set(&verified_username, &session.key)?;
        let account_key = account_key(&verified_username);
        let (same_account, was_enabled, previous_username) = {
            let runtime = self.runtime.lock().await;
            (
                runtime.account_key.as_deref() == Some(account_key.as_str()),
                runtime.enabled,
                runtime.username.clone(),
            )
        };
        if !same_account {
            if let Some(previous_username) = previous_username.as_deref() {
                self.session_store.delete(previous_username)?;
            }
        }
        db.set_setting(SETTING_LASTFM_USERNAME, &verified_username)?;
        db.set_setting(SETTING_LASTFM_ACCOUNT_KEY, &account_key)?;
        db.set_setting(
            SETTING_LASTFM_ENABLED,
            if same_account && was_enabled {
                "true"
            } else {
                "false"
            },
        )?;
        db.set_setting(SETTING_LASTFM_NEEDS_REAUTH, "false")?;
        {
            let mut runtime = self.runtime.lock().await;
            runtime.username = Some(verified_username);
            runtime.account_key = Some(account_key);
            runtime.session_key = Some(session.key);
            runtime.enabled = same_account && was_enabled;
            runtime.pending = None;
            runtime.auth_state = LastFmAuthState::Connected;
            runtime.last_error = None;
            runtime.active_attempts.clear();
        }
        self.wake();
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub async fn cancel_auth(&self, db: &Db, app: &AppHandle) -> AppResult<LastFmStatus> {
        {
            let mut runtime = self.runtime.lock().await;
            runtime.pending = None;
            runtime.auth_state = if runtime.session_key.is_some() {
                if db.get_setting(SETTING_LASTFM_NEEDS_REAUTH)?.as_deref() == Some("true") {
                    LastFmAuthState::NeedsReauthentication
                } else {
                    LastFmAuthState::Connected
                }
            } else {
                LastFmAuthState::Disconnected
            };
            runtime.last_error = None;
        }
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub(super) async fn verify_restored_session(&self, db: &Db, app: &AppHandle) {
        let (old_username, session_key) = {
            let runtime = self.runtime.lock().await;
            if runtime.auth_state != LastFmAuthState::Connected {
                return;
            }
            let (Some(username), Some(session_key)) =
                (runtime.username.clone(), runtime.session_key.clone())
            else {
                return;
            };
            (username, session_key)
        };
        let Ok(client) = self.client() else {
            return;
        };
        match client.verify_user(&session_key).await {
            Ok(username) => {
                let key = account_key(&username);
                if username != old_username
                    && self.session_store.set(&username, &session_key).is_ok()
                {
                    let _ = self.session_store.delete(&old_username);
                }
                if db.set_setting(SETTING_LASTFM_USERNAME, &username).is_ok()
                    && db.set_setting(SETTING_LASTFM_ACCOUNT_KEY, &key).is_ok()
                {
                    let mut runtime = self.runtime.lock().await;
                    runtime.username = Some(username);
                    runtime.account_key = Some(key);
                    runtime.last_error = None;
                }
            }
            Err(LastFmError::Api(9)) => {
                let _ = self.mark_needs_reauth(db).await;
            }
            Err(error) => {
                self.runtime.lock().await.last_error = Some(issue(
                    "verification",
                    api_code(&error),
                    error.safe_message(),
                ));
            }
        }
        self.emit_status(db, app).await;
    }

    pub(super) async fn mark_needs_reauth(&self, db: &Db) -> AppResult<()> {
        db.set_setting(SETTING_LASTFM_NEEDS_REAUTH, "true")?;
        let mut runtime = self.runtime.lock().await;
        runtime.auth_state = LastFmAuthState::NeedsReauthentication;
        runtime.active_attempts.clear();
        runtime.last_error = Some(issue(
            "reauthentication",
            Some(9),
            "Reconnect Last.fm to resume scrobbling.",
        ));
        Ok(())
    }

    async fn set_error(&self, error: LastFmError, connected: bool) {
        let mut runtime = self.runtime.lock().await;
        runtime.auth_state = if connected {
            LastFmAuthState::Connected
        } else {
            LastFmAuthState::Error
        };
        runtime.last_error = Some(issue("api", api_code(&error), error.safe_message()));
    }
}
