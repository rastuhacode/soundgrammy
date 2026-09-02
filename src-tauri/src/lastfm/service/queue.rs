use super::*;

impl LastFmService {
    pub async fn worker_loop(app: AppHandle) {
        {
            let state = app.state::<AppState>();
            state.lastfm.verify_restored_session(&state.db, &app).await;
        }
        loop {
            let state = app.state::<AppState>();
            if let Err(error) = state.lastfm.flush_once(&state.db, &app).await {
                tracing::warn!("Last.fm queue pass failed: {error}");
            }
            let wait_ms = state
                .lastfm
                .next_worker_wait_ms(&state.db)
                .await
                .unwrap_or(15 * 60 * 1000);
            tokio::select! {
                _ = state.lastfm.notify.notified() => {},
                _ = tokio::time::sleep(Duration::from_millis(wait_ms as u64)) => {},
            }
        }
    }

    pub async fn flush_once(&self, db: &Db, app: &AppHandle) -> AppResult<()> {
        let _worker = self.worker_lock.lock().await;
        let (account_key, session_key) = {
            let runtime = self.runtime.lock().await;
            if !runtime.enabled || runtime.auth_state != LastFmAuthState::Connected {
                return Ok(());
            }
            let (Some(account_key), Some(session_key)) =
                (runtime.account_key.clone(), runtime.session_key.clone())
            else {
                return Ok(());
            };
            (account_key, session_key)
        };
        let rows = db.due_lastfm_scrobbles(&account_key, now_ms(), 50)?;
        if rows.is_empty() {
            return Ok(());
        }
        let payload = rows
            .iter()
            .map(|row| LastFmScrobble {
                artist: row.artist.clone(),
                track: row.track_title.clone(),
                duration: row.duration_seconds,
                timestamp: row.started_at_utc,
            })
            .collect::<Vec<_>>();
        let client = self.client()?;
        match client.scrobble(&session_key, &payload).await {
            Ok(results) => {
                let mut delete_ids = Vec::new();
                let mut daily_ids = Vec::new();
                let mut accepted = false;
                for (row, result) in rows.iter().zip(results) {
                    match result.ignored_code {
                        0 => {
                            accepted = true;
                            delete_ids.push(row.id);
                        }
                        5 => daily_ids.push(row.id),
                        code => {
                            delete_ids.push(row.id);
                            let mut runtime = self.runtime.lock().await;
                            runtime.last_metadata_warning = Some(issue(
                                "filtered",
                                Some(code),
                                "Last.fm permanently filtered a scrobble.",
                            ));
                        }
                    }
                }
                db.delete_lastfm_queue_rows(&delete_ids)?;
                if !daily_ids.is_empty() {
                    db.retry_lastfm_queue_rows(
                        &daily_ids,
                        now_ms() + 24 * 60 * 60 * 1000,
                        Some(5),
                        "Daily scrobble limit exceeded",
                    )?;
                }
                if accepted {
                    db.set_setting(SETTING_LASTFM_LAST_SCROBBLE_AT_MS, &now_ms().to_string())?;
                    self.runtime.lock().await.last_error = None;
                }
            }
            Err(LastFmError::Api(9)) => self.mark_needs_reauth(db).await?,
            Err(error @ (LastFmError::Transport | LastFmError::Malformed))
            | Err(error @ LastFmError::Api(11 | 16)) => {
                retry_all(db, &rows, &error, 30_000, 3_600_000)?;
                self.runtime.lock().await.last_error =
                    Some(issue("transport", api_code(&error), error.safe_message()));
            }
            Err(error @ LastFmError::Api(29)) => {
                retry_all(db, &rows, &error, 300_000, 21_600_000)?;
                self.runtime.lock().await.last_error =
                    Some(issue("rate_limit", Some(29), error.safe_message()));
            }
            Err(error) => {
                let mut runtime = self.runtime.lock().await;
                runtime.auth_state = LastFmAuthState::Error;
                runtime.active_attempts.clear();
                runtime.last_error = Some(issue("api", api_code(&error), error.safe_message()));
            }
        }
        self.emit_status(db, app).await;
        Ok(())
    }

    async fn next_worker_wait_ms(&self, db: &Db) -> AppResult<i64> {
        let runtime = self.runtime.lock().await;
        if !runtime.enabled || runtime.auth_state != LastFmAuthState::Connected {
            return Ok(15 * 60 * 1000);
        }
        let account_key = runtime.account_key.clone();
        drop(runtime);
        let Some(account_key) = account_key else {
            return Ok(15 * 60 * 1000);
        };
        if !db
            .due_lastfm_scrobbles(&account_key, now_ms(), 1)?
            .is_empty()
        {
            return Ok(250);
        }
        Ok(db
            .next_lastfm_attempt_at(&account_key)?
            .map(|due| (due - now_ms()).max(250))
            .unwrap_or(15 * 60 * 1000))
    }
}

fn retry_all(
    db: &Db,
    rows: &[crate::db::LastFmQueueRow],
    error: &LastFmError,
    base_ms: i64,
    cap_ms: i64,
) -> AppResult<()> {
    let exponent = rows
        .iter()
        .map(|row| row.attempt_count)
        .max()
        .unwrap_or(0)
        .clamp(0, 20) as u32;
    let delay = base_ms
        .saturating_mul(2_i64.saturating_pow(exponent))
        .min(cap_ms);
    let jitter_percent = 80 + (now_ms().unsigned_abs() % 41) as i64;
    let due = now_ms() + delay.saturating_mul(jitter_percent) / 100;
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    db.retry_lastfm_queue_rows(&ids, due, api_code(error), error.safe_message())
}
