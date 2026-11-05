//! Playback accounting follows PCM consumption, never WebView timers or seek position.
use super::{Pipeline, Request};
use crate::{listen_stats::EndReason, state::AppState};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

enum AccountingEvent {
    Start(crate::lastfm::models::PlaybackStart),
    Qualify(String, i64),
    End(String),
    LocalStart(i64, u64),
    LocalEnd(i64, i64, Option<i64>, EndReason, u64),
    Preferences(String),
    ListenSettings {
        enabled: Option<bool>,
        clear: bool,
        session_epoch: u64,
        reply: tokio::sync::oneshot::Sender<Result<super::Snapshot, String>>,
    },
}

#[derive(Default)]
struct FrameClock {
    generation: u64,
    frames: u64,
    nanos: u128,
}
impl FrameClock {
    fn sample(&mut self, generation: u64, frames: u64, rate: u32) {
        if generation != self.generation {
            self.generation = generation;
            self.frames = 0;
        }
        self.nanos +=
            frames.saturating_sub(self.frames) as u128 * 1_000_000_000 / rate.max(1) as u128;
        self.frames = frames;
    }
    fn ms(&self) -> i64 {
        (self.nanos / 1_000_000).min(i64::MAX as u128) as i64
    }
}
struct Attempt {
    id: String,
    track: i64,
    duration: Option<i64>,
    // Match the metadata snapshot used by LastFmService, not decoder duration.
    lastfm_duration: Option<i64>,
    clock: FrameClock,
    local_baseline: Option<i64>,
    lastfm_baseline: Option<i64>,
    qualified: bool,
    lastfm_started: bool,
    settings_epoch: u64,
    lastfm_epoch: u64,
}
pub struct Activity {
    current: Option<Attempt>,
    lastfm: tokio::sync::mpsc::UnboundedSender<(u64, AccountingEvent)>,
    last_check: Instant,
    nonce: u128,
    sequence: u64,
}
impl Activity {
    pub fn new(app: &AppHandle) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let app = app.clone();
        // One ordered sink: qualification cannot overtake start or be deleted by end.
        // Only attempt edges enter this queue; PCM/timer ticks never accumulate here.
        tauri::async_runtime::spawn(async move {
            while let Some((epoch, event)) = rx.recv().await {
                let state = app.state::<AppState>();
                if epoch != state.audio.lastfm_epoch.load(Ordering::Acquire)
                    && matches!(
                        &event,
                        AccountingEvent::Start(_) | AccountingEvent::Qualify(_, _)
                    )
                {
                    continue;
                }
                let result = match event {
                    AccountingEvent::Start(start) => {
                        state.lastfm.register_started(&state.db, &app, start).await
                    }
                    AccountingEvent::Qualify(id, ms) => {
                        state
                            .lastfm
                            .attempt_qualified(&state.db, &app, &id, ms)
                            .await
                    }
                    AccountingEvent::LocalStart(track, epoch) => {
                        let app = app.clone();
                        state
                            .db
                            .blocking(move |db| {
                                if epoch
                                    != app
                                        .state::<AppState>()
                                        .audio
                                        .accounting_epoch
                                        .load(Ordering::Acquire)
                                {
                                    return Ok(());
                                }
                                db.record_attempt_start(track)
                            })
                            .await
                    }
                    AccountingEvent::LocalEnd(track, ms, duration, reason, epoch) => {
                        let app = app.clone();
                        state
                            .db
                            .blocking(move |db| {
                                if epoch
                                    != app
                                        .state::<AppState>()
                                        .audio
                                        .accounting_epoch
                                        .load(Ordering::Acquire)
                                {
                                    return Ok(());
                                }
                                let result = db.record_attempt_end(track, ms, duration, reason)?;
                                let _ = app.emit("listen:stats", result.stats);
                                Ok(())
                            })
                            .await
                    }
                    AccountingEvent::ListenSettings {
                        enabled,
                        clear,
                        session_epoch,
                        reply,
                    } => {
                        let app = app.clone();
                        let result = state
                            .db
                            .blocking(move |db| {
                                let state = app.state::<AppState>();
                                if state.audio.epochs.session.load(Ordering::Acquire)
                                    != session_epoch
                                {
                                    return Err(crate::error::AppError::NotAuthorized);
                                }
                                if clear {
                                    db.clear_listen_stats_for_user(db.active_uid()?)?;
                                }
                                if let Some(enabled) = enabled {
                                    db.set_setting(
                                        crate::db::SETTING_LISTEN_STATS_ENABLED,
                                        if enabled { "true" } else { "false" },
                                    )?;
                                    state
                                        .audio
                                        .local_listens_enabled
                                        .store(enabled, Ordering::Release);
                                }
                                state.audio.accounting_epoch.fetch_add(1, Ordering::AcqRel);
                                Ok(state.audio.snapshot())
                            })
                            .await
                            .map_err(|error| error.to_string());
                        let _ = reply.send(result);
                        Ok(())
                    }
                    AccountingEvent::Preferences(raw) => {
                        state
                            .db
                            .blocking(move |db| db.set_setting("playback_preferences", &raw))
                            .await
                    }
                    AccountingEvent::End(id) => {
                        state.lastfm.attempt_ended(&id).await;
                        Ok(())
                    }
                };
                if let Err(error) = result {
                    tracing::warn!(%error, "native playback accounting failed");
                }
            }
        });
        Self {
            current: None,
            lastfm: tx,
            last_check: Instant::now(),
            sequence: 0,
            nonce: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        }
    }
    pub fn update_listen_settings(
        &self,
        enabled: Option<bool>,
        clear: bool,
        session_epoch: u64,
        reply: tokio::sync::oneshot::Sender<Result<super::Snapshot, String>>,
    ) {
        let _ = self.lastfm.send((
            0,
            AccountingEvent::ListenSettings {
                enabled,
                clear,
                session_epoch,
                reply,
            },
        ));
    }
    pub fn save_preferences(&self, preferences: &super::session::Preferences) {
        if let Ok(raw) = serde_json::to_string(preferences) {
            let _ = self.lastfm.send((0, AccountingEvent::Preferences(raw)));
        }
    }
    pub fn start(&mut self, app: &AppHandle, request: &Request) {
        let state = app.state::<AppState>();
        self.sequence += 1;
        self.current = Some(Attempt {
            id: format!("{}:{}:{}", self.nonce, self.sequence, request.attempt_id),
            track: request.track_id,
            duration: request
                .expected_duration_seconds
                .filter(|d| *d > 0.0)
                .map(|d| (d * 1000.0) as i64),
            lastfm_duration: request
                .expected_duration_seconds
                .filter(|d| *d > 0.0)
                .map(|d| d as i64 * 1000),
            clock: FrameClock::default(),
            local_baseline: None,
            lastfm_baseline: None,
            qualified: false,
            lastfm_started: false,
            settings_epoch: state.audio.accounting_epoch.load(Ordering::Acquire),
            lastfm_epoch: state.audio.lastfm_epoch.load(Ordering::Acquire),
        });
        self.settings(app);
    }
    pub fn settings(&mut self, app: &AppHandle) {
        let Some(a) = self.current.as_mut() else {
            return;
        };
        let state = app.state::<AppState>();
        let lastfm_epoch = state.audio.lastfm_epoch.load(Ordering::Acquire);
        if lastfm_epoch != a.lastfm_epoch {
            if a.lastfm_started {
                let _ = self
                    .lastfm
                    .send((a.lastfm_epoch, AccountingEvent::End(a.id.clone())));
            }
            a.lastfm_epoch = lastfm_epoch;
            a.lastfm_started = false;
            a.lastfm_baseline = None;
        }
        let epoch = state.audio.accounting_epoch.load(Ordering::Acquire);
        if epoch != a.settings_epoch {
            a.local_baseline = None;
            a.settings_epoch = epoch;
        }
        let enabled = state.audio.local_listens_enabled.load(Ordering::Acquire);
        if !enabled {
            a.local_baseline = None;
        } else if a.local_baseline.is_none() {
            let _ = self
                .lastfm
                .send((0, AccountingEvent::LocalStart(a.track, a.settings_epoch)));
            a.local_baseline = Some(a.clock.ms());
        }
        let enabled = state.audio.lastfm_enabled.load(Ordering::Acquire);
        if !enabled {
            if a.lastfm_started {
                let _ = self
                    .lastfm
                    .send((a.lastfm_epoch, AccountingEvent::End(a.id.clone())));
            }
            a.lastfm_started = false;
            a.lastfm_baseline = None;
        } else if a.lastfm_baseline.is_none() {
            a.lastfm_baseline = Some(a.clock.ms());
            a.qualified = false;
        }
    }
    pub fn ensure_started(&mut self, app: &AppHandle, request: &Request) {
        if self.current.is_none() {
            self.start(app, request);
        }
    }

    pub fn sample(
        &mut self,
        app: &AppHandle,
        pipeline: Option<&Pipeline>,
        generation: u64,
        duration: f64,
    ) {
        if let (Some(a), Some(p)) = (&mut self.current, pipeline) {
            a.clock.sample(
                generation,
                p.output.clock.rendered.load(Ordering::Acquire),
                p.output.rate,
            );
            if duration > 0.0 {
                a.duration = Some((duration * 1000.0) as i64);
            }
        }
        let state = app.state::<AppState>();
        let settings_changed = self
            .current
            .as_ref()
            .is_some_and(|a| a.lastfm_epoch != state.audio.lastfm_epoch.load(Ordering::Acquire));
        if settings_changed || self.last_check.elapsed() >= Duration::from_secs(1) {
            self.settings(app);
            self.last_check = Instant::now();
        }
        if let Some(a) = self.current.as_mut() {
            if a.lastfm_baseline.is_some() && !a.lastfm_started && a.clock.ms() > 0 {
                a.lastfm_started = true;
                let _ = self.lastfm.send((
                    a.lastfm_epoch,
                    AccountingEvent::Start(crate::lastfm::models::PlaybackStart {
                        attempt_id: a.id.clone(),
                        track_id: a.track,
                        started_at_utc: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                        duration_seconds: a.lastfm_duration.map(|d| d / 1000),
                    }),
                ));
            }
        }
        self.qualify();
    }
    fn qualify(&mut self) {
        let Some(a) = self.current.as_mut() else {
            return;
        };
        if !a.lastfm_started {
            return;
        }
        let Some(baseline) = a.lastfm_baseline else {
            return;
        };
        let Some(threshold) =
            crate::lastfm::service::qualification_threshold_ms(a.lastfm_duration.map(|d| d / 1000))
        else {
            return;
        };
        let ms = a.clock.ms().saturating_sub(baseline);
        if !a.qualified && ms >= threshold {
            a.qualified = true;
            let _ = self
                .lastfm
                .send((a.lastfm_epoch, AccountingEvent::Qualify(a.id.clone(), ms)));
        }
    }
    pub fn finish(&mut self, app: &AppHandle, reason: EndReason) {
        self.settings(app);
        self.qualify();
        let Some(a) = self.current.take() else { return };
        if let Some(baseline) = a.local_baseline {
            let _ = self.lastfm.send((
                0,
                AccountingEvent::LocalEnd(
                    a.track,
                    a.clock.ms().saturating_sub(baseline),
                    a.duration,
                    reason,
                    a.settings_epoch,
                ),
            ));
        }
        if a.lastfm_started {
            let _ = self
                .lastfm
                .send((a.lastfm_epoch, AccountingEvent::End(a.id)));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_lastfm_qualification_is_once_per_attempt_and_ignores_short_tracks() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut activity = Activity {
            current: Some(Attempt {
                id: "attempt-1".into(),
                track: 1,
                duration: Some(119_000),
                lastfm_duration: Some(120_000),
                clock: FrameClock::default(),
                local_baseline: Some(0),
                lastfm_baseline: Some(0),
                qualified: false,
                lastfm_started: true,
                settings_epoch: 0,
                lastfm_epoch: 0,
            }),
            lastfm: tx,
            last_check: Instant::now(),
            nonce: 1,
            sequence: 1,
        };
        activity
            .current
            .as_mut()
            .unwrap()
            .clock
            .sample(1, 59 * 48_000, 48_000);
        activity.qualify();
        assert!(rx.try_recv().is_err());
        activity
            .current
            .as_mut()
            .unwrap()
            .clock
            .sample(1, 60 * 48_000, 48_000);
        activity.qualify();
        activity.qualify();
        assert!(
            matches!(rx.try_recv().unwrap(), (0, AccountingEvent::Qualify(id, 60_000)) if id == "attempt-1")
        );
        assert!(rx.try_recv().is_err());
        let a = activity.current.as_mut().unwrap();
        a.id = "attempt-2".into();
        a.lastfm_duration = Some(30_000);
        a.qualified = false;
        activity.qualify();
        assert!(rx.try_recv().is_err());
        activity.current = None;
        activity.qualify();
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn seeks_and_buffering_do_not_inflate_pcm_time() {
        let mut c = FrameClock::default();
        c.sample(1, 48_000, 48_000);
        assert_eq!(c.ms(), 1000);
        // Seek changes neither the lifetime frame counter nor accounting time.
        c.sample(1, 48_000, 48_000);
        c.sample(1, 96_000, 48_000);
        assert_eq!(c.ms(), 2000);
        // Output recreation may change sample rate and resets the counter.
        c.sample(2, 44_100, 44_100);
        c.sample(2, 44_100, 44_100);
        assert_eq!(c.ms(), 3000);
    }
}
