use super::*;

impl LastFmService {
    pub async fn start_auth(&self, db: &Db, app: &dyn LastFmHost) -> AppResult<LastFmStatus> {
        let client = self.client()?;
        let generation = {
            let mut runtime = self.runtime.lock().await;
            runtime.generation += 1;
            runtime.auth_state = LastFmAuthState::RequestingToken;
            runtime.pending = None;
            runtime.last_error = None;
            runtime.generation
        };
        self.emit_status(db, app).await;
        let result = client.get_token().await;
        let mut runtime = self.runtime.lock().await;
        if runtime.generation != generation {
            drop(runtime);
            return self.status(db).await;
        }
        let url = match result {
            Ok(token) => {
                let url = client
                    .authorization_url(&token)
                    .map_err(|_| AppError::msg("Last.fm authorization could not be started."))?;
                runtime.pending = Some(PendingToken {
                    token,
                    requested_at_ms: now_ms(),
                });
                runtime.auth_state = LastFmAuthState::WaitingForBrowserApproval;
                Some(url)
            }
            Err(error) => {
                set_runtime_error(&mut runtime, error);
                None
            }
        };
        if let Some(url) = url {
            if app.open_url(&url).is_err() {
                runtime.pending = None;
                runtime.auth_state = LastFmAuthState::Error;
                runtime.last_error = Some(issue(
                    "authorization",
                    None,
                    "The Last.fm authorization page could not be opened.",
                ));
            }
        }
        drop(runtime);
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub async fn complete_auth(&self, db: &Db, app: &dyn LastFmHost) -> AppResult<LastFmStatus> {
        let client = self.client()?;
        let (pending, generation) = {
            let mut runtime = self.runtime.lock().await;
            let Some(pending) = runtime.pending.clone() else {
                drop(runtime);
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
            (pending, runtime.generation)
        };
        self.emit_status(db, app).await;
        let result = client.get_session(&pending.token).await;
        let result = match result {
            Ok(mut session) => {
                match client.verify_user(&session.key).await {
                    Ok(username) => session.username = username,
                    Err(LastFmError::Api(9)) => {
                        return self
                            .auth_failure(db, app, generation, LastFmError::Api(9))
                            .await
                    }
                    Err(_) => {} // Session exchange supplies canonical identity on transient verification failures.
                }
                Ok(session)
            }
            Err(error) => Err(error),
        };
        let mut runtime = self.runtime.lock().await;
        if runtime.generation != generation {
            drop(runtime);
            return self.status(db).await;
        }
        match result {
            Err(LastFmError::Api(14)) => {
                runtime.auth_state = LastFmAuthState::WaitingForBrowserApproval
            }
            Err(error) => {
                if error == LastFmError::Api(15) {
                    runtime.pending = None;
                }
                set_runtime_error(&mut runtime, error);
            }
            Ok(session) => {
                let key = account_key(&session.username);
                let same_account = runtime.account_key.as_deref() == Some(&key);
                let enabled = same_account && runtime.enabled;
                let persistence = (|| {
                    let previous_key = self.session_store.get(&session.username)?;
                    self.session_store.set(&session.username, &session.key)?;
                    if let Err(error) = db.update_settings(&[
                        (SETTING_LASTFM_USERNAME, Some(&session.username)),
                        (SETTING_LASTFM_ACCOUNT_KEY, Some(&key)),
                        (
                            SETTING_LASTFM_ENABLED,
                            Some(if enabled { "true" } else { "false" }),
                        ),
                        (SETTING_LASTFM_NEEDS_REAUTH, Some("false")),
                    ]) {
                        let recovery = match previous_key {
                            Some(previous) => self.session_store.set(&session.username, &previous),
                            None => self.session_store.delete(&session.username),
                        };
                        if recovery.is_err() {
                            return Err(AppError::CredentialRecovery);
                        }
                        return Err(error);
                    }
                    Ok(())
                })();
                if let Err(error) = persistence {
                    if matches!(error, AppError::CredentialRecovery) {
                        runtime.auth_state = LastFmAuthState::NeedsReauthentication;
                        runtime.enabled = false;
                        runtime.session_key = None;
                        runtime.active_attempts.clear();
                        app.invalidate_accounting();
                    } else {
                        runtime.auth_state = if runtime.session_key.is_some() {
                            LastFmAuthState::Connected
                        } else {
                            LastFmAuthState::WaitingForBrowserApproval
                        };
                    }
                    runtime.last_error = Some(issue(
                        "storage",
                        None,
                        "Last.fm account settings could not be saved. Retry or reconnect.",
                    ));
                    drop(runtime);
                    self.emit_status(db, app).await;
                    return Err(error);
                }
                if !same_account {
                    if let Some(previous) = runtime.username.as_deref() {
                        if let Err(error) = self.session_store.delete(previous) {
                            tracing::warn!(%error,"Could not remove superseded Last.fm credential");
                        }
                    }
                }
                runtime.generation += 1;
                runtime.username = Some(session.username);
                runtime.account_key = Some(key);
                runtime.session_key = Some(session.key);
                runtime.enabled = enabled;
                runtime.pending = None;
                runtime.auth_state = LastFmAuthState::Connected;
                runtime.last_error = None;
                runtime.active_attempts.clear();
                app.invalidate_accounting();
                self.wake();
            }
        }
        drop(runtime);
        self.emit_status(db, app).await;
        self.status(db).await
    }

    async fn auth_failure(
        &self,
        db: &Db,
        app: &dyn LastFmHost,
        generation: u64,
        error: LastFmError,
    ) -> AppResult<LastFmStatus> {
        let mut runtime = self.runtime.lock().await;
        if runtime.generation == generation {
            set_runtime_error(&mut runtime, error);
        }
        drop(runtime);
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub async fn cancel_auth(&self, db: &Db, app: &dyn LastFmHost) -> AppResult<LastFmStatus> {
        {
            let mut runtime = self.runtime.lock().await;
            runtime.generation += 1;
            runtime.pending = None;
            runtime.auth_state = if runtime.session_key.is_some() {
                if db.get_setting(SETTING_LASTFM_NEEDS_REAUTH)?.as_deref() == Some("true") {
                    LastFmAuthState::NeedsReauthentication
                } else {
                    LastFmAuthState::Connected
                }
            } else if runtime.username.is_some() {
                LastFmAuthState::NeedsReauthentication
            } else {
                LastFmAuthState::Disconnected
            };
            runtime.last_error = None;
        }
        self.emit_status(db, app).await;
        self.status(db).await
    }

    pub(super) async fn verify_restored_session(&self, db: &Db, app: &dyn LastFmHost) {
        let (generation, old_username, session_key) = {
            let runtime = self.runtime.lock().await;
            if runtime.auth_state != LastFmAuthState::Connected {
                return;
            }
            let (Some(username), Some(key)) = (&runtime.username, &runtime.session_key) else {
                return;
            };
            (runtime.generation, username.clone(), key.clone())
        };
        let Ok(client) = self.client() else {
            return;
        };
        let result = client.verify_user(&session_key).await;
        let mut runtime = self.runtime.lock().await;
        if runtime.generation != generation {
            return;
        }
        match result {
            Ok(username) if username == old_username => runtime.last_error = None,
            Ok(_) => {
                // A restored credential must not silently move the persisted queue to another identity.
                let _ = mark_runtime_needs_reauth(&mut runtime, db);
            }
            Err(LastFmError::Api(9)) => {
                let _ = mark_runtime_needs_reauth(&mut runtime, db);
            }
            Err(error) => {
                runtime.last_error = Some(issue(
                    "verification",
                    api_code(&error),
                    error.safe_message(),
                ))
            }
        }
        drop(runtime);
        self.emit_status(db, app).await;
    }
}

pub(super) fn mark_runtime_needs_reauth(runtime: &mut RuntimeState, db: &Db) -> AppResult<()> {
    db.set_setting(SETTING_LASTFM_NEEDS_REAUTH, "true")?;
    runtime.auth_state = LastFmAuthState::NeedsReauthentication;
    runtime.active_attempts.clear();
    runtime.last_error = Some(issue(
        "reauthentication",
        Some(9),
        "Reconnect Last.fm to resume scrobbling.",
    ));
    Ok(())
}

fn set_runtime_error(runtime: &mut RuntimeState, error: LastFmError) {
    runtime.auth_state = LastFmAuthState::Error;
    runtime.last_error = Some(issue("api", api_code(&error), error.safe_message()));
}
