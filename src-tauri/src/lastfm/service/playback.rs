use super::*;

impl LastFmService {
    pub async fn attempt_started(
        &self,
        db: &Db,
        app: &AppHandle,
        attempt_id: String,
        track_id: i64,
    ) -> AppResult<()> {
        if attempt_id.trim().is_empty() || db.lastfm_attempt_is_queued(&attempt_id)? {
            return Ok(());
        }
        let (username, account_key, session_key) = {
            let runtime = self.runtime.lock().await;
            if !runtime.enabled || runtime.auth_state != LastFmAuthState::Connected {
                return Ok(());
            }
            let (Some(username), Some(account_key), Some(session_key)) = (
                runtime.username.clone(),
                runtime.account_key.clone(),
                runtime.session_key.clone(),
            ) else {
                return Ok(());
            };
            (username, account_key, session_key)
        };
        let Some(profile) = db.load_profile()? else {
            return Ok(());
        };
        let Some(track) = db.track_by_id(track_id, profile.tg_user_id)? else {
            return Ok(());
        };
        let (artist, title) = match eligible_metadata(
            track.performer.as_deref(),
            track.title.as_deref(),
            &track.title_source,
        ) {
            Ok(metadata) => metadata,
            Err(message) => {
                let mut runtime = self.runtime.lock().await;
                runtime.last_metadata_warning = Some(issue("metadata", None, message));
                drop(runtime);
                self.emit_status(db, app).await;
                return Ok(());
            }
        };
        let snapshot = PlaybackSnapshot {
            attempt_id: attempt_id.clone(),
            username,
            account_key,
            track_id,
            artist: artist.to_owned(),
            track_title: title.to_owned(),
            duration_seconds: track.duration.filter(|value| *value > 0),
            started_at_utc: now_seconds(),
        };
        {
            let mut runtime = self.runtime.lock().await;
            if runtime.active_attempts.contains_key(&attempt_id) {
                return Ok(());
            }
            runtime.active_attempts.insert(attempt_id, snapshot.clone());
        }
        let client = self.client()?;
        match client
            .update_now_playing(
                &session_key,
                &snapshot.artist,
                &snapshot.track_title,
                snapshot.duration_seconds,
            )
            .await
        {
            Ok(0) => {}
            Ok(code) => {
                let mut runtime = self.runtime.lock().await;
                runtime.last_metadata_warning = Some(issue(
                    "filtered",
                    Some(code),
                    "Last.fm filtered the Now Playing update.",
                ));
            }
            Err(LastFmError::Api(9)) => self.mark_needs_reauth(db).await?,
            Err(error) => {
                let mut runtime = self.runtime.lock().await;
                runtime.last_error =
                    Some(issue("now_playing", api_code(&error), error.safe_message()));
            }
        }
        self.emit_status(db, app).await;
        Ok(())
    }

    pub async fn attempt_qualified(
        &self,
        db: &Db,
        app: &AppHandle,
        attempt_id: &str,
        listened_ms: i64,
    ) -> AppResult<()> {
        let snapshot = {
            let runtime = self.runtime.lock().await;
            if !runtime.enabled || runtime.auth_state != LastFmAuthState::Connected {
                return Ok(());
            }
            runtime.active_attempts.get(attempt_id).cloned()
        };
        let Some(snapshot) = snapshot else {
            return Ok(());
        };
        let Some(threshold) = qualification_threshold_ms(snapshot.duration_seconds) else {
            return Ok(());
        };
        if listened_ms < threshold {
            return Ok(());
        }
        let inserted = db.enqueue_lastfm_scrobble(&LastFmQueueInsert {
            attempt_id: snapshot.attempt_id,
            username: snapshot.username,
            account_key: snapshot.account_key,
            track_id: Some(snapshot.track_id),
            artist: snapshot.artist,
            track_title: snapshot.track_title,
            duration_seconds: snapshot.duration_seconds,
            started_at_utc: snapshot.started_at_utc,
            created_at_ms: now_ms(),
        })?;
        if inserted {
            self.wake();
            self.emit_status(db, app).await;
        }
        Ok(())
    }

    pub async fn attempt_ended(&self, attempt_id: &str) {
        self.runtime.lock().await.active_attempts.remove(attempt_id);
    }
}

fn eligible_metadata<'a>(
    artist: Option<&'a str>,
    title: Option<&'a str>,
    title_source: &str,
) -> Result<(&'a str, &'a str), &'static str> {
    let artist = artist.map(str::trim).filter(|value| !value.is_empty());
    let title = title.map(str::trim).filter(|value| !value.is_empty());
    if title_source != "telegram_audio" {
        return Err("Filename-derived track titles are not sent to Last.fm.");
    }
    let artist = artist.ok_or("A structured artist is required for Last.fm.")?;
    let title = title.ok_or("A structured track title is required for Last.fm.")?;
    if artist.eq_ignore_ascii_case("unknown artist") {
        return Err("Placeholder artists are not sent to Last.fm.");
    }
    if title.eq_ignore_ascii_case("unknown track") {
        return Err("Placeholder track titles are not sent to Last.fm.");
    }
    Ok((artist, title))
}

fn qualification_threshold_ms(duration_seconds: Option<i64>) -> Option<i64> {
    match duration_seconds {
        Some(duration) if duration <= 30 => None,
        Some(duration) => Some((duration * 1000 / 2).min(240_000)),
        None => Some(240_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_requires_structured_non_placeholder_fields() {
        assert!(eligible_metadata(Some("Artist"), Some("Track"), "telegram_audio").is_ok());
        assert!(
            eligible_metadata(Some("Unknown Artist"), Some("Track"), "telegram_audio").is_err()
        );
        assert!(
            eligible_metadata(Some("Artist"), Some("Unknown Track"), "telegram_audio").is_err()
        );
        assert!(eligible_metadata(Some("Artist"), Some("Track.mp3"), "filename").is_err());
        assert!(eligible_metadata(Some("  "), Some("Track"), "telegram_audio").is_err());
    }

    #[test]
    fn scrobble_threshold_matches_lastfm_rules() {
        assert_eq!(qualification_threshold_ms(Some(30)), None);
        assert_eq!(qualification_threshold_ms(Some(31)), Some(15_500));
        assert_eq!(qualification_threshold_ms(Some(900)), Some(240_000));
        assert_eq!(qualification_threshold_ms(None), Some(240_000));
    }
}
