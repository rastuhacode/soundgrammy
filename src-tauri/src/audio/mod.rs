//! Native transport and authoritative playback session.
mod activity;
#[cfg(target_os = "android")]
mod android;
mod availability;
mod convert;
mod decode;
#[cfg(target_os = "ios")]
mod ios;
pub mod lifecycle;
pub mod media;
mod output;
mod seek;
pub mod session;
mod shuffle;
mod source;

use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub track_id: i64,
    pub attempt_id: String,
    pub expected_duration_seconds: Option<f64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AudioError {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Range {
    start: f64,
    end: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    last_control: Option<String>,
    pub revision: u64,
    kind: &'static str,
    status: &'static str,
    track_id: Option<i64>,
    attempt_id: Option<String>,
    current_time_seconds: f64,
    duration_seconds: f64,
    buffered_ranges: Vec<Range>,
    volume_percent: f64,
    error: Option<AudioError>,
    initial_loading: bool,
    seeking: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    player: Option<Arc<session::Session>>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            last_control: None,
            revision: 0,
            kind: "native-rust",
            status: "idle",
            track_id: None,
            attempt_id: None,
            current_time_seconds: 0.0,
            duration_seconds: 0.0,
            buffered_ranges: Vec::new(),
            volume_percent: 100.0,
            error: None,
            initial_loading: false,
            seeking: false,
            player: Some(Arc::new(session::Session::default())),
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Ended {
    r#type: &'static str,
    track_id: i64,
    attempt_id: String,
    revision: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub available: bool,
    pub streaming: bool,
}
pub enum Control {
    Player(Box<session::Command>),
    Remote(media::QueuedRemote),
    #[allow(dead_code)] // OS callbacks are mobile-only.
    Lifecycle(lifecycle::Event),
    ListenSettings {
        enabled: Option<bool>,
        clear: bool,
    },
    CacheStream(i64, Arc<crate::streaming::TrackStream>),
    CacheReady(i64),
    Stop,
    Complete(u64),
    Sync,
    Start(Request, bool, f64),
    Unload,
    Play,
    Pause,
    Seek(f64),
    SeekAttempt(f64, Option<String>),
    Volume(f64),
    Capabilities,
}
impl From<session::Command> for Control {
    fn from(command: session::Command) -> Self {
        Self::Player(Box::new(command))
    }
}
struct Envelope {
    page_epoch: u64,
    session_epoch: u64,
    control: Control,
    reply: tokio::sync::oneshot::Sender<Result<Snapshot, String>>,
}
impl Envelope {
    fn valid_for(&self, page: u64, session: u64) -> bool {
        if self.session_epoch != session {
            return false;
        }
        matches!(self.control, Control::Remote(_) | Control::Lifecycle(_))
            || self.page_epoch == page
            || matches!(self.control, Control::Complete(_))
            || matches!(
                self.control,
                Control::CacheStream(_, _) | Control::CacheReady(_)
            )
            || matches!(&self.control, Control::Player(command) if matches!(command.as_ref(), session::Command::Clear))
    }
}
#[derive(Default)]
struct CommandEpochs {
    page: AtomicU64,
    session: AtomicU64,
}
#[derive(Default)]
pub struct NativeAudioService {
    pub accounting_epoch: AtomicU64,
    pub lastfm_epoch: AtomicU64,
    worker: Mutex<Option<(SyncSender<Envelope>, JoinHandle<()>)>>,
    snapshot: Arc<Mutex<Snapshot>>,
    stopping: Arc<AtomicBool>,
    clearing: Arc<AtomicBool>,
    epochs: Arc<CommandEpochs>,
    protection: Arc<Mutex<Option<(u64, PathBuf)>>>,
}
impl NativeAudioService {
    pub async fn observe_cache(
        &self,
        app: AppHandle,
        track_id: i64,
        stream: Arc<crate::streaming::TrackStream>,
    ) {
        if self.snapshot().track_id == Some(track_id) {
            let _ = self
                .command(app, Control::CacheStream(track_id, stream))
                .await;
        }
    }
    pub async fn cache_ready(&self, app: AppHandle, track_id: i64) {
        if self.snapshot().track_id == Some(track_id) {
            let _ = self.command(app, Control::CacheReady(track_id)).await;
        }
    }
    pub fn clear_session(&self) {
        self.epochs.session.fetch_add(1, Ordering::AcqRel);
        self.epochs.page.fetch_add(1, Ordering::AcqRel);
        self.clearing.store(true, Ordering::Release);
    }
    pub fn page_loading(&self) {
        self.epochs.page.fetch_add(1, Ordering::AcqRel);
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn protect(&self, generation: u64, path: PathBuf) {
        let mut protection = self.protection.lock().unwrap_or_else(|p| p.into_inner());
        if protection
            .as_ref()
            .is_some_and(|(current, _)| generation == *current)
        {
            *protection = Some((generation, path));
        }
    }
    pub fn protected_paths(&self) -> Vec<PathBuf> {
        self.protection
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|(_, p)| vec![p.clone()])
            .unwrap_or_default()
    }
    pub async fn command(&self, app: AppHandle, control: Control) -> Result<Snapshot, String> {
        #[cfg(target_os = "android")]
        android::initialize(&app).await?;
        if self.stopping.load(Ordering::Acquire) {
            return Err("interrupted".into());
        }
        let (reply, receive) = tokio::sync::oneshot::channel();
        {
            let mut worker = self.worker.lock().unwrap_or_else(|p| p.into_inner());
            if worker.is_none() {
                let (tx, rx) = mpsc::sync_channel(32);
                let shared = self.snapshot.clone();
                let stopping = self.stopping.clone();
                let clearing = self.clearing.clone();
                let epochs = self.epochs.clone();
                let protection = self.protection.clone();
                let thread = std::thread::Builder::new()
                    .name("native-audio-control".into())
                    .spawn(move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            run(
                                app.clone(),
                                rx,
                                shared.clone(),
                                stopping.clone(),
                                clearing,
                                epochs,
                                protection,
                            )
                        }));
                        if result.is_err() && !stopping.load(Ordering::Acquire) {
                            let snapshot = {
                                let mut snapshot = shared.lock().unwrap_or_else(|p| p.into_inner());
                                fail(&mut snapshot, "interrupted", None);
                                snapshot.revision += 1;
                                snapshot.clone()
                            };
                            media::publish(&app, &snapshot, true);
                            let _ = app.emit("audio:state", snapshot.clone());
                        }
                    })
                    .map_err(|_| "output-unavailable")?;
                *worker = Some((tx, thread));
            }
            worker
                .as_ref()
                .unwrap()
                .0
                .try_send(Envelope {
                    control,
                    reply,
                    page_epoch: self.epochs.page.load(Ordering::Acquire),
                    session_epoch: self.epochs.session.load(Ordering::Acquire),
                })
                .map_err(|_| "audio command queue unavailable")?;
        }
        receive.await.map_err(|_| "interrupted".to_string())?
    }
    /// Enqueue synchronously from native callbacks, preserving arrival order with UI commands.
    pub fn remote_command(&self, command: media::RemoteCommand) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) || self.clearing.load(Ordering::Acquire) {
            return Err("interrupted".into());
        }
        let command = media::QueuedRemote::capture(command, &self.snapshot());
        let (reply, _) = tokio::sync::oneshot::channel();
        self.worker
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .ok_or("audio worker unavailable")?
            .0
            .try_send(Envelope {
                page_epoch: self.epochs.page.load(Ordering::Acquire),
                session_epoch: self.epochs.session.load(Ordering::Acquire),
                control: Control::Remote(command),
                reply,
            })
            .map_err(|_| "audio command queue unavailable".into())
    }
    /// Native OS callbacks are process-owned and survive WebView replacement.
    #[allow(dead_code)] // OS callbacks are mobile-only.
    pub fn lifecycle_event(&self, event: lifecycle::Event) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) || self.clearing.load(Ordering::Acquire) {
            return Err("interrupted".into());
        }
        let (reply, _) = tokio::sync::oneshot::channel();
        self.worker
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .ok_or("audio worker unavailable")?
            .0
            .try_send(Envelope {
                page_epoch: self.epochs.page.load(Ordering::Acquire),
                session_epoch: self.epochs.session.load(Ordering::Acquire),
                control: Control::Lifecycle(event),
                reply,
            })
            .map_err(|_| "audio command queue unavailable".into())
    }
    pub fn shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        if let Some((_, worker)) = self.worker.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}
fn fail(snapshot: &mut Snapshot, code: &str, diagnostic: Option<String>) {
    tracing::warn!(
        engine = "native-rust",
        track_id = snapshot.track_id,
        code,
        diagnostic,
        "audio playback failed"
    );
    snapshot.seeking = false;
    snapshot.status = "error";
    snapshot.initial_loading = false;
    snapshot.error = Some(AudioError {
        code: code.into(),
        message: "Native audio playback failed. Try playing the track again.".into(),
        recoverable: true,
        diagnostic,
    });
}
struct Pipeline {
    diagnostic_stage: String,
    output: output::Output,
    active: Arc<AtomicBool>,
    decoder: Option<JoinHandle<()>>,
    messages: Receiver<decode::Message>,
    eof: bool,
    origin: f64,
    landed: bool,
    seek: Arc<seek::SeekControl>,
    seek_revision: u64,
    waiting_seek: bool,
    seek_started: Option<Instant>,
    availability: Arc<availability::Availability>,
    observer: Option<CoverageObserver>,
    source_diagnostic: Arc<Mutex<Option<String>>>,
}
// The packet scanner can outlive mobile audio output while a paused track caches.
struct CoverageObserver {
    active: Arc<AtomicBool>,
    availability: Arc<availability::Availability>,
}
impl Drop for CoverageObserver {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        self.availability.scan_seek.cancel_read();
    }
}
fn refresh_paused_buffer(snapshot: &mut Snapshot, observer: Option<&CoverageObserver>) -> bool {
    let Some(observer) = observer else {
        return false;
    };
    let ranges = observer.availability.ranges(snapshot.duration_seconds);
    if snapshot.buffered_ranges == ranges {
        return false;
    }
    snapshot.buffered_ranges = ranges;
    true
}
fn reconnect_cache_observer(
    snapshot: &Snapshot,
    has_pipeline: bool,
    paused: &mut Option<CoverageObserver>,
    track_id: i64,
    spawn: impl FnOnce(Arc<AtomicBool>, Arc<availability::Availability>) -> std::io::Result<()>,
) -> bool {
    if snapshot.track_id != Some(track_id) || snapshot.status != "paused" || has_pipeline {
        return false;
    }
    let availability = Arc::new(availability::Availability::default());
    if let Some(previous) = paused.as_ref() {
        for range in previous.availability.ranges(snapshot.duration_seconds) {
            availability.record(range.start, range.end);
        }
    }
    let active = Arc::new(AtomicBool::new(true));
    if spawn(active.clone(), availability.clone()).is_err() {
        return false;
    }
    *paused = Some(CoverageObserver {
        active,
        availability,
    });
    true
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        self.output.clock.playing.store(false, Ordering::Release);
        self.active.store(false, Ordering::Release);
        self.seek.cancel_read();
        if let Some(worker) = self.decoder.take() {
            let deadline = Instant::now() + Duration::from_millis(250);
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}
fn pipeline(
    app: AppHandle,
    request: &Request,
    generation: u64,
    origin: f64,
    volume: f64,
) -> Result<Pipeline, &'static str> {
    let (output, producer) = output::Output::open(volume)?;
    let active = Arc::new(AtomicBool::new(true));
    let token = active.clone();
    let (tx, messages) = mpsc::sync_channel(8);
    let track = request.track_id;
    let rate = output.rate;
    let channels = output.channels;
    let clock = output.clock.clone();
    let seek = Arc::new(seek::SeekControl::default());
    let availability = Arc::new(availability::Availability::default());
    let observer_active = Arc::new(AtomicBool::new(true));
    let source_diagnostic = Arc::new(Mutex::new(None));
    let decoder_seek = seek.clone();
    let decoder_availability = availability.clone();
    let decoder_observer_active = observer_active.clone();
    let decoder_diagnostic = source_diagnostic.clone();
    let decoder = std::thread::Builder::new()
        .name("native-audio-decode".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let source = source::Source::open(
                    app,
                    track,
                    generation,
                    token.clone(),
                    decoder_observer_active,
                    decoder_seek.clone(),
                    decoder_availability.clone(),
                    decoder_diagnostic,
                )
                .map_err(|_| "source-unavailable")?;
                decode::decode_inner(
                    Box::new(source),
                    origin,
                    (rate, channels),
                    producer,
                    token.clone(),
                    clock,
                    tx.clone(),
                    Some(decode::Live {
                        seek: decoder_seek,
                        availability: decoder_availability,
                    }),
                )
            }))
            .unwrap_or(Err("decode-failed"));
            if token.load(Ordering::Acquire) {
                if let Err(code) = result {
                    let _ = tx.send(decode::Message::Failed(code));
                }
            }
        })
        .map_err(|_| "interrupted")?;
    Ok(Pipeline {
        diagnostic_stage: "opening source".into(),
        output,
        active,
        decoder: Some(decoder),
        messages,
        eof: false,
        origin,
        landed: false,
        seek,
        seek_revision: 0,
        waiting_seek: false,
        seek_started: None,
        availability: availability.clone(),
        observer: Some(CoverageObserver {
            active: observer_active,
            availability,
        }),
        source_diagnostic,
    })
}
fn update_session(snapshot: &mut Snapshot, session: &mut session::Session, desired: bool) {
    if session.is_playing != desired {
        session.is_playing = desired;
        session.revision += 1;
    }
    if snapshot.player.as_ref().is_none_or(|p| {
        p.revision != session.revision
            || p.preferences.repeat != session.preferences.repeat
            || p.preferences.shuffle != session.preferences.shuffle
            || p.preferences.mode != session.preferences.mode
    }) {
        snapshot.player = Some(Arc::new(session.clone()));
    }
}
fn session_control(
    effect: session::Effect,
    session: &session::Session,
    snapshot: &Snapshot,
) -> Control {
    match effect {
        session::Effect::None => Control::Sync,
        session::Effect::Unload => Control::Unload,
        session::Effect::Play => Control::Play,
        session::Effect::Pause => Control::Pause,
        session::Effect::Seek(seconds) => Control::Seek(seconds),
        session::Effect::Load => {
            let track = session.queue.current().unwrap();
            // Only an explicit seek after final completion keeps its position on replay.
            let origin = if session.end_reason == "completed"
                && snapshot.status != "ended"
                && snapshot.track_id == Some(track.id)
                && snapshot.current_time_seconds < snapshot.duration_seconds
            {
                snapshot.current_time_seconds
            } else {
                0.0
            };
            Control::Start(
                Request {
                    track_id: track.id,
                    attempt_id: format!("native:{}", session.attempt),
                    expected_duration_seconds: track.duration.map(|d| d.max(0) as f64),
                },
                session.is_playing,
                origin,
            )
        }
    }
}
fn run(
    app: AppHandle,
    rx: Receiver<Envelope>,
    shared: Arc<Mutex<Snapshot>>,
    stopping: Arc<AtomicBool>,
    clearing: Arc<AtomicBool>,
    epochs: Arc<CommandEpochs>,
    protection: Arc<Mutex<Option<(u64, PathBuf)>>>,
) {
    let mut snapshot = Snapshot::default();
    let mut request: Option<Request> = None;
    let mut current: Option<Pipeline> = None;
    let mut paused_observer: Option<CoverageObserver> = None;
    let mut generation = 0;
    let mut desired = false;
    let mut lifecycle = lifecycle::Policy::default();
    let mut ended = false;
    let mut last_emit = Instant::now();
    let mut last_consumed = 0;
    let mut stalled_since = Instant::now();
    let mut current_page = 0;
    let state = app.state::<crate::state::AppState>();
    let stored_preferences = state.db.get_setting("playback_preferences").ok().flatten();
    let mut session = session::Session::with_preferences(stored_preferences.as_deref());
    let mut activity = activity::Activity::new(&app);
    let mut pending_completion = None;
    while !stopping.load(Ordering::Acquire) {
        activity.sample(
            &app,
            current.as_ref(),
            generation,
            snapshot.duration_seconds,
        );
        let mut envelope = if clearing.swap(false, Ordering::AcqRel) {
            pending_completion = None;
            let (reply, _) = tokio::sync::oneshot::channel();
            Some(Envelope {
                control: Control::from(session::Command::Clear),
                reply,
                page_epoch: epochs.page.load(Ordering::Acquire),
                session_epoch: epochs.session.load(Ordering::Acquire),
            })
        } else if let Some(attempt) = pending_completion.take() {
            let (reply, _) = tokio::sync::oneshot::channel();
            Some(Envelope {
                control: Control::Complete(attempt),
                reply,
                page_epoch: epochs.page.load(Ordering::Acquire),
                session_epoch: epochs.session.load(Ordering::Acquire),
            })
        } else {
            rx.recv_timeout(Duration::from_millis(20)).ok()
        };
        let page = epochs.page.load(Ordering::Acquire);
        // Navigation invalidates queued UI commands, not the native session.
        if page != current_page {
            current_page = page;
        }
        if envelope
            .as_ref()
            .is_some_and(|e| !e.valid_for(current_page, epochs.session.load(Ordering::Acquire)))
        {
            envelope = None;
        }
        let mut dirty = envelope.is_some();
        if let Some(envelope) = envelope {
            let mut restart = None;
            let from_remote = matches!(&envelope.control, Control::Remote(_));
            let incoming = match envelope.control {
                Control::Remote(command) => {
                    snapshot.last_control = Some(format!("remote: {command:?}"));
                    command.control(&snapshot).unwrap_or(Control::Sync)
                }
                other => other,
            };
            let control = match incoming {
                Control::Player(command) => {
                    let command = *command;
                    // Record only the variant name; queue payloads contain Telegram metadata.
                    let name = match &command {
                        session::Command::Playing { playing: true } => "play",
                        session::Command::Playing { playing: false } => "pause",
                        session::Command::Toggle => "toggle",
                        session::Command::Next => "next",
                        session::Command::Previous { .. } => "previous",
                        session::Command::Attach { .. } => "attach",
                        _ => "queue/preferences",
                    };
                    if name != "attach" && !from_remote {
                        snapshot.last_control = Some(format!("ui: {name}"));
                    }
                    let saves_preferences = command.saves_preferences();
                    let effect =
                        session.apply(command, snapshot.current_time_seconds, |entries, mode| {
                            shuffle::shuffle(entries, mode, &state.db)
                        });
                    let effect = match effect {
                        Ok(effect) => effect,
                        Err(error) => {
                            let _ = envelope.reply.send(Err(error));
                            continue;
                        }
                    };
                    if saves_preferences {
                        if let Ok(raw) = serde_json::to_string(&session.preferences) {
                            if let Err(error) = state.db.set_setting("playback_preferences", &raw) {
                                tracing::warn!(%error, "Could not save playback preferences");
                            }
                        }
                    }
                    session_control(effect, &session, &snapshot)
                }
                Control::SeekAttempt(seconds, attempt) => {
                    snapshot.last_control =
                        Some(format!("ui seek: seconds={seconds}, attempt={attempt:?}"));
                    if attempt.is_some() && attempt != snapshot.attempt_id {
                        let _ = envelope
                            .reply
                            .send(Err("Playback changed before seeking".into()));
                        continue;
                    }
                    Control::Seek(seconds)
                }
                Control::Complete(attempt) => {
                    snapshot.last_control = Some(format!("natural completion: {attempt}"));
                    let effect = session.complete(attempt);
                    session_control(effect, &session, &snapshot)
                }
                other => other,
            };
            match control {
                Control::Remote(_)
                | Control::Player(_)
                | Control::Complete(_)
                | Control::SeekAttempt(_, _) => {
                    unreachable!()
                }
                Control::Stop => {
                    lifecycle.apply(lifecycle::Event::PermanentLoss);
                    activity.finish(&app, crate::listen_stats::EndReason::Stopped);
                    desired = false;
                    current = None;
                    paused_observer = None;
                    snapshot.current_time_seconds = 0.0;
                    snapshot.status = "paused";
                    snapshot.seeking = false;
                    snapshot.initial_loading = false;
                    snapshot.error = None;
                    snapshot.buffered_ranges.clear();
                }
                Control::Lifecycle(event) => {
                    if lifecycle.apply(event) {
                        desired = false;
                        if let Some(p) = current.as_mut() {
                            paused_observer = p.observer.take();
                        }
                        current = None;
                    }
                    if !lifecycle.blocked() && desired && current.is_none() {
                        restart = Some(snapshot.current_time_seconds);
                    }
                    if lifecycle.blocked() || !desired {
                        if let Some(p) = current.as_ref() {
                            p.output.clock.playing.store(false, Ordering::Release);
                        }
                        snapshot.status = "paused";
                    }
                }
                Control::ListenSettings { enabled, clear } => {
                    let result = if clear {
                        state.db.clear_listen_stats()
                    } else if let Some(enabled) = enabled {
                        state.db.set_setting(
                            crate::db::SETTING_LISTEN_STATS_ENABLED,
                            if enabled { "true" } else { "false" },
                        )
                    } else {
                        Ok(())
                    };
                    if let Err(error) = result {
                        let _ = envelope.reply.send(Err(error.to_string()));
                        continue;
                    }
                    state.audio.accounting_epoch.fetch_add(1, Ordering::AcqRel);
                    activity.settings(&app);
                }
                Control::CacheStream(track_id, stream) => {
                    reconnect_cache_observer(
                        &snapshot,
                        current.is_some(),
                        &mut paused_observer,
                        track_id,
                        |active, availability| {
                            source::spawn_observer(
                                stream,
                                active,
                                availability,
                                Arc::new(Mutex::new(None)),
                            )
                        },
                    );
                }
                Control::CacheReady(track_id) => {
                    if snapshot.track_id == Some(track_id) && snapshot.duration_seconds > 0.0 {
                        if let Some(p) = current.as_ref() {
                            p.availability.complete.store(true, Ordering::Release);
                        }
                        if let Some(p) = paused_observer.as_ref() {
                            p.availability.complete.store(true, Ordering::Release);
                        }
                        snapshot.buffered_ranges = vec![Range {
                            start: 0.0,
                            end: snapshot.duration_seconds,
                        }];
                    }
                }
                Control::Sync => {
                    if snapshot.status == "ended" && !session.is_playing {
                        current = None;
                    }
                }
                Control::Start(next, playing, origin) => {
                    activity.finish(
                        &app,
                        crate::listen_stats::EndReason::parse(session.end_reason)
                            .unwrap_or(crate::listen_stats::EndReason::Replaced),
                    );
                    activity.start(&app, &next);
                    desired = playing;
                    ended = false;
                    snapshot.track_id = Some(next.track_id);
                    snapshot.attempt_id = Some(next.attempt_id.clone());
                    snapshot.duration_seconds = next.expected_duration_seconds.unwrap_or(0.0);
                    snapshot.current_time_seconds = origin;
                    snapshot.seeking = false;
                    request = Some(next);
                    restart = Some(origin);
                }
                Control::Capabilities => {
                    let result = if current.is_some() {
                        Ok(snapshot.clone())
                    } else {
                        output::Output::open(snapshot.volume_percent)
                            .map(|_| snapshot.clone())
                            .map_err(str::to_string)
                    };
                    #[cfg(target_os = "ios")]
                    if current.is_none() {
                        ios::deactivate();
                    }
                    let _ = envelope.reply.send(result);
                    continue;
                }
                Control::Unload => {
                    lifecycle.apply(lifecycle::Event::PermanentLoss);
                    activity.finish(&app, crate::listen_stats::EndReason::Stopped);
                    current = None;
                    paused_observer = None;
                    request = None;
                    desired = false;
                    ended = false;
                    snapshot = Snapshot {
                        revision: snapshot.revision,
                        volume_percent: snapshot.volume_percent,
                        ..Default::default()
                    };
                    *protection.lock().unwrap_or_else(|p| p.into_inner()) = None;
                }
                Control::Play => {
                    if let Some(request) = request.as_ref() {
                        activity.ensure_started(&app, request);
                        desired = true;
                        if snapshot.status == "error" || current.is_none() {
                            restart = Some(snapshot.current_time_seconds);
                        }
                        snapshot.status = "buffering";
                    }
                }
                Control::Pause => {
                    desired = false;
                    #[cfg(target_os = "android")]
                    lifecycle.apply(lifecycle::Event::PermanentLoss);
                    if let Some(p) = current.as_ref() {
                        p.output.clock.playing.store(false, Ordering::Release);
                    }
                    if cfg!(any(target_os = "android", target_os = "ios"))
                        || matches!(snapshot.status, "loading" | "buffering")
                    {
                        // Keep mapping verified chunks after releasing the output pipeline.
                        if let Some(p) = current.as_mut() {
                            paused_observer = p.observer.take();
                        }
                        current = None;
                    }
                    if request.is_some() && snapshot.status != "error" {
                        snapshot.status = "paused";
                        snapshot.seeking = false;
                        snapshot.initial_loading = false;
                    }
                }
                Control::Seek(seconds) => {
                    if request.is_some() {
                        let target = if snapshot.duration_seconds > 0.0 {
                            seconds.clamp(0.0, snapshot.duration_seconds)
                        } else {
                            seconds.max(0.0)
                        };
                        snapshot.current_time_seconds = target;
                        snapshot.status = "buffering";
                        snapshot.seeking = true;
                        snapshot.initial_loading = false;
                        if let Some(p) = current.as_mut() {
                            p.output.clock.playing.store(false, Ordering::Release);
                            p.seek_revision = p.seek.request(target);
                            p.availability.scan_seek.request(target);
                            p.origin = target;
                            p.landed = false;
                            p.waiting_seek = true;
                            p.seek_started = Some(Instant::now());
                            p.eof = false;
                            last_consumed = 0;
                            stalled_since = Instant::now();
                        } else {
                            restart = Some(target);
                        }
                    }
                }
                Control::Volume(value) => {
                    snapshot.volume_percent = value.clamp(0.0, 100.0);
                    if let Some(p) = current.as_ref() {
                        p.output.clock.gain.store(
                            (snapshot.volume_percent as f32 / 100.0).to_bits(),
                            Ordering::Relaxed,
                        );
                    }
                }
            }
            #[cfg(target_os = "android")]
            if desired && !lifecycle.blocked() && !media::platform::acquire() {
                desired = false;
                current = None;
                restart = None;
                activity.finish(&app, crate::listen_stats::EndReason::Interrupted);
                fail(
                    &mut snapshot,
                    "output-unavailable",
                    Some("Android foreground service or audio focus unavailable".into()),
                );
            }
            if let Some(origin) = restart {
                paused_observer = None;
                current = None;
                generation += 1;
                // Install a generation watermark before async source resolution.
                *protection.lock().unwrap_or_else(|p| p.into_inner()) =
                    Some((generation, PathBuf::new()));
                snapshot.status = if origin == 0.0 {
                    "loading"
                } else {
                    "buffering"
                };
                snapshot.initial_loading = origin == 0.0;
                snapshot.error = None;
                snapshot.buffered_ranges.clear();
                last_consumed = 0;
                stalled_since = Instant::now();
                match pipeline(
                    app.clone(),
                    request.as_ref().unwrap(),
                    generation,
                    origin,
                    snapshot.volume_percent,
                ) {
                    Ok(p) => current = Some(p),
                    Err(code) => {
                        fail(&mut snapshot, code, None);
                        desired = false;
                    }
                }
            }
            #[cfg(target_os = "ios")]
            if current.is_none() && !desired {
                ios::deactivate();
            }
            update_session(&mut snapshot, &mut session, desired);
            snapshot.revision += 1;
            *shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
            media::publish(&app, &snapshot, true);
            let _ = app.emit("audio:state", snapshot.clone());
            let _ = envelope.reply.send(Ok(snapshot.clone()));
        }
        let mut error = None;
        let audible = desired && !lifecycle.blocked();
        if let Some(p) = current.as_mut() {
            while let Ok(message) = p.messages.try_recv() {
                dirty = true;
                match message {
                    decode::Message::Diagnostic(stage) => p.diagnostic_stage = stage,
                    decode::Message::Duration(d) if d.is_finite() && d > 0.0 => {
                        snapshot.duration_seconds = d
                    }
                    decode::Message::Duration(_) => {}
                    decode::Message::Eof(revision) if revision == p.seek_revision => p.eof = true,
                    decode::Message::Seeked(revision) if revision == p.seek_revision => {
                        p.waiting_seek = false;
                        stalled_since = Instant::now();
                    }
                    decode::Message::Eof(_) | decode::Message::Seeked(_) => {}
                    decode::Message::Failed(code) => {
                        let diagnostic = p
                            .source_diagnostic
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .take();
                        error = Some((
                            code,
                            Some(format!(
                                "stage={}; source={}",
                                p.diagnostic_stage,
                                diagnostic
                                    .as_deref()
                                    .unwrap_or("no source IO error recorded")
                            )),
                        ));
                    }
                }
            }
            let clock = &p.output.clock;
            if clock.failed.load(Ordering::Acquire) {
                error = Some(("output-unavailable", None));
            }
            let consumed = clock.consumed.load(Ordering::Relaxed);
            let queued = clock
                .produced
                .load(Ordering::Acquire)
                .saturating_sub(consumed);
            let presented = clock.presented.load(Ordering::Acquire);
            if presented != last_consumed {
                stalled_since = Instant::now();
                last_consumed = presented;
            }
            snapshot.buffered_ranges = p.availability.ranges(snapshot.duration_seconds);
            if !p.waiting_seek {
                if p.eof && queued == 0 && consumed == 0 && snapshot.seeking {
                    snapshot.seeking = false;
                    snapshot.status = "paused";
                    desired = false;
                }
                let ready = queued >= p.output.rate as u64 / 20 || (p.eof && queued > 0);
                if ready {
                    dirty |= snapshot.seeking;
                    if let Some(started) = p.seek_started.take() {
                        tracing::debug!(
                            seek_revision = p.seek_revision,
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "native seek ready for output"
                        );
                    }
                    snapshot.seeking = false;
                }
                if audible && ready && snapshot.status != "playing" {
                    clock.playing.store(true, Ordering::Release);
                }
                if audible && presented > 0 && stalled_since.elapsed() < Duration::from_millis(150)
                {
                    if snapshot.status != "playing" {
                        dirty = true;
                    }
                    snapshot.status = "playing";
                    snapshot.initial_loading = false;
                } else if audible && stalled_since.elapsed() >= Duration::from_millis(150) {
                    if snapshot.status != "buffering" {
                        dirty = true;
                    }
                    snapshot.status = "buffering";
                    if !ready {
                        clock.playing.store(false, Ordering::Release);
                    }
                } else if !audible && ready && matches!(snapshot.status, "loading" | "buffering") {
                    snapshot.status = if snapshot.initial_loading {
                        "ready"
                    } else {
                        "paused"
                    };
                    snapshot.initial_loading = false;
                    dirty = true;
                }
                if (audible && presented > 0) || (!p.landed && ready) {
                    let position = p.origin + presented as f64 / p.output.rate as f64;
                    snapshot.current_time_seconds = if !p.landed {
                        position
                    } else {
                        position.max(snapshot.current_time_seconds)
                    };
                    p.landed = true;
                }
                if snapshot.duration_seconds > 0.0 {
                    snapshot.current_time_seconds =
                        snapshot.current_time_seconds.min(snapshot.duration_seconds);
                }
                if p.eof
                    && queued == 0
                    && consumed > 0
                    && presented >= consumed
                    && audible
                    && !ended
                    && error.is_none()
                {
                    activity.sample(&app, Some(p), generation, snapshot.duration_seconds);
                    activity.finish(&app, crate::listen_stats::EndReason::Completed);
                    ended = true;
                    pending_completion = Some(session.attempt);
                    desired = false;
                    clock.playing.store(false, Ordering::Release);
                    snapshot.status = "ended";
                    snapshot.initial_loading = false;
                    dirty = true;
                    snapshot.revision += 1;
                    *shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
                    media::publish(&app, &snapshot, true);
                    let _ = app.emit("audio:state", snapshot.clone());
                    let _ = app.emit(
                        "audio:event",
                        Ended {
                            r#type: "ended",
                            track_id: snapshot.track_id.unwrap(),
                            attempt_id: snapshot.attempt_id.clone().unwrap(),
                            revision: snapshot.revision,
                        },
                    );
                }
            }
        }
        dirty |= refresh_paused_buffer(&mut snapshot, paused_observer.as_ref());
        if let Some((code, diagnostic)) = error {
            current = None;
            desired = false;
            activity.finish(&app, crate::listen_stats::EndReason::Interrupted);
            fail(&mut snapshot, code, diagnostic);
            #[cfg(target_os = "ios")]
            ios::deactivate();
            dirty = true;
        }
        if dirty || (current.is_some() && last_emit.elapsed() >= Duration::from_millis(150)) {
            if pending_completion.is_none() {
                update_session(&mut snapshot, &mut session, desired);
            }
            snapshot.revision += 1;
            *shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
            let mut event = snapshot.clone();
            if !dirty {
                event.player = None;
            }
            media::publish(&app, &snapshot, dirty);
            let _ = app.emit("audio:state", event);
            last_emit = Instant::now();
        }
    }
    activity.sample(
        &app,
        current.as_ref(),
        generation,
        snapshot.duration_seconds,
    );
    activity.finish(&app, crate::listen_stats::EndReason::Interrupted);
    drop(current);
    drop(paused_observer);
    #[cfg(target_os = "ios")]
    ios::deactivate();
    *protection.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detached_cache_scanner_advances_a_paused_native_buffer() {
        let availability = Arc::new(availability::Availability::default());
        availability.record(0.0, 5.0);
        let old_active = Arc::new(AtomicBool::new(true));
        let mut paused = Some(CoverageObserver {
            active: old_active.clone(),
            availability,
        });
        let mut snapshot = Snapshot {
            status: "paused",
            track_id: Some(7),
            duration_seconds: 120.0,
            buffered_ranges: vec![Range {
                start: 0.0,
                end: 5.0,
            }],
            ..Default::default()
        };
        assert!(!reconnect_cache_observer(
            &snapshot,
            false,
            &mut paused,
            8,
            |_, _| unreachable!("a different track cannot replace the observer"),
        ));
        let mut worker = None;
        assert!(reconnect_cache_observer(
            &snapshot,
            false,
            &mut paused,
            7,
            |active, coverage| {
                worker = Some(source::spawn_scanner(
                    Box::new(std::io::Cursor::new(
                        include_bytes!("../../tests/fixtures/audio/long-tone.mp3").to_vec(),
                    )),
                    active,
                    coverage,
                )?);
                Ok(())
            },
        ));
        assert!(!old_active.load(Ordering::Acquire));
        let deadline = Instant::now() + Duration::from_secs(3);
        while snapshot.buffered_ranges[0].end <= 60.0 && Instant::now() < deadline {
            refresh_paused_buffer(&mut snapshot, paused.as_ref());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(snapshot.buffered_ranges[0].end > 60.0);
        assert_eq!(snapshot.status, "paused");
        drop(paused);
        worker.unwrap().join().unwrap();
    }
    #[test]
    fn paused_mobile_observer_keeps_updating_buffer_without_playback() {
        let availability = Arc::new(availability::Availability::default());
        let active = Arc::new(AtomicBool::new(true));
        let observer = CoverageObserver {
            active: active.clone(),
            availability: availability.clone(),
        };
        let mut snapshot = Snapshot {
            status: "paused",
            duration_seconds: 120.0,
            buffered_ranges: vec![Range {
                start: 0.0,
                end: 5.0,
            }],
            ..Default::default()
        };

        availability.record(0.0, 5.0);
        assert!(!refresh_paused_buffer(&mut snapshot, Some(&observer)));
        availability.record(5.0, 50.0);
        assert!(refresh_paused_buffer(&mut snapshot, Some(&observer)));
        assert_eq!(snapshot.status, "paused");
        assert_eq!(snapshot.buffered_ranges[0].end, 50.0);
        assert!(active.load(Ordering::Acquire));

        availability.complete.store(true, Ordering::Release);
        assert!(refresh_paused_buffer(&mut snapshot, Some(&observer)));
        assert_eq!(snapshot.buffered_ranges[0].end, 120.0);
        drop(observer);
        assert!(!active.load(Ordering::Acquire));
    }
    #[test]
    fn page_recreation_invalidates_ui_commands_but_not_completion_or_logout() {
        let envelope = |control| {
            let (reply, _) = tokio::sync::oneshot::channel();
            Envelope {
                page_epoch: 1,
                session_epoch: 0,
                control,
                reply,
            }
        };
        assert!(!envelope(Control::Play).valid_for(2, 0));
        assert!(envelope(Control::Remote(media::QueuedRemote::capture(
            media::RemoteCommand::Next,
            &Snapshot::default(),
        )))
        .valid_for(2, 0));
        assert!(envelope(Control::Complete(3)).valid_for(2, 0));
        assert!(envelope(Control::from(session::Command::Clear)).valid_for(2, 0));
        assert!(!envelope(Control::Remote(media::QueuedRemote::capture(
            media::RemoteCommand::Next,
            &Snapshot::default(),
        )))
        .valid_for(2, 1));
        let service = NativeAudioService::default();
        service.page_loading();
        assert!(!service.clearing.load(Ordering::Acquire));
        assert!(!service.stopping.load(Ordering::Acquire));
        service.clear_session();
        assert!(service.clearing.load(Ordering::Acquire));
    }
    #[test]
    fn native_callbacks_queue_in_order_and_stop_after_logout_or_shutdown() {
        let service = NativeAudioService::default();
        let (tx, rx) = mpsc::sync_channel(4);
        *service.worker.lock().unwrap() = Some((tx, std::thread::spawn(|| {})));
        service.remote_command(media::RemoteCommand::Next).unwrap();
        service.page_loading();
        service.remote_command(media::RemoteCommand::Pause).unwrap();
        let first = rx.try_recv().unwrap();
        let second = rx.try_recv().unwrap();
        assert!(matches!(first.control, Control::Remote(_)));
        assert!(matches!(second.control, Control::Remote(_)));
        assert!(first.valid_for(1, 0));
        assert!(second.valid_for(1, 0));
        service.clear_session();
        assert!(!first.valid_for(1, service.epochs.session.load(Ordering::Acquire)));
        assert!(service.remote_command(media::RemoteCommand::Play).is_err());
        service.shutdown();
        assert!(service.remote_command(media::RemoteCommand::Next).is_err());
        assert!(service.worker.lock().unwrap().is_none());
    }
    #[test]
    fn lifecycle_callbacks_survive_page_recreation_but_not_logout() {
        let service = NativeAudioService::default();
        let (tx, rx) = mpsc::sync_channel(4);
        *service.worker.lock().unwrap() = Some((tx, std::thread::spawn(|| {})));
        service.lifecycle_event(lifecycle::Event::Begin(1)).unwrap();
        service.page_loading();
        service.remote_command(media::RemoteCommand::Pause).unwrap();
        service
            .lifecycle_event(lifecycle::Event::End(1, true))
            .unwrap();
        let begin = rx.try_recv().unwrap();
        let pause = rx.try_recv().unwrap();
        let end = rx.try_recv().unwrap();
        assert!(begin.valid_for(1, 0));
        assert!(pause.valid_for(1, 0));
        assert!(end.valid_for(1, 0));
        assert!(matches!(
            begin.control,
            Control::Lifecycle(lifecycle::Event::Begin(1))
        ));
        assert!(matches!(pause.control, Control::Remote(_)));
        assert!(matches!(
            end.control,
            Control::Lifecycle(lifecycle::Event::End(1, true))
        ));
        service.clear_session();
        assert!(!end.valid_for(1, 1));
        assert!(service
            .lifecycle_event(lifecycle::Event::End(1, true))
            .is_err());
        service.shutdown();
    }
    #[test]
    fn completed_replay_preserves_only_an_explicit_post_end_seek() {
        let track = serde_json::from_value(serde_json::json!({"id":1,"tg_user_id":1,"file_id":"","file_unique_id":"","title":null,"performer":null,"duration":120,"source":"saved_music","mime_type":null,"file_size":null,"created_at":""})).unwrap();
        let mut session = session::Session::default();
        session
            .apply(
                session::Command::SetQueue {
                    queue: session::Queue {
                        tracks: vec![track],
                        cursor: 0,
                        ..Default::default()
                    },
                    play: true,
                },
                0.0,
                |e, _| e,
            )
            .unwrap();
        session.complete(session.attempt);
        let effect = session
            .apply(session::Command::Playing { playing: true }, 40.0, |e, _| e)
            .unwrap();
        let snapshot = Snapshot {
            track_id: Some(1),
            current_time_seconds: 40.0,
            duration_seconds: 120.0,
            status: "paused",
            ..Default::default()
        };
        assert!(matches!(
            session_control(effect, &session, &snapshot),
            Control::Start(_, true, 40.0)
        ));
        let snapshot = Snapshot {
            status: "ended",
            ..snapshot
        };
        assert!(matches!(
            session_control(session::Effect::Load, &session, &snapshot),
            Control::Start(_, true, 0.0)
        ));
    }
    #[test]
    fn late_source_resolution_cannot_replace_current_cache_protection() {
        let service = NativeAudioService::default();
        *service.protection.lock().unwrap() = Some((1, PathBuf::new()));
        service.protect(1, PathBuf::from("a"));
        assert_eq!(service.protected_paths(), vec![PathBuf::from("a")]);
        *service.protection.lock().unwrap() = Some((3, PathBuf::new()));
        service.protect(3, PathBuf::from("a-new-attempt"));
        service.protect(1, PathBuf::from("a"));
        service.protect(2, PathBuf::from("b"));
        assert_eq!(
            service.protected_paths(),
            vec![PathBuf::from("a-new-attempt")]
        );
        *service.protection.lock().unwrap() = None;
        service.protect(3, PathBuf::from("a-new-attempt"));
        assert!(service.protected_paths().is_empty());
    }
}
