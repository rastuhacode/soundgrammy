//! Desktop transport service. Queue and listening policy remain in TypeScript.
mod availability;
mod convert;
mod decode;
mod output;
mod seek;
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
use tauri::{AppHandle, Emitter};

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
}
#[derive(Clone, Debug, Serialize)]
pub struct Range {
    start: f64,
    end: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
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
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
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
    Load(Request),
    Unload,
    Play,
    Pause,
    Seek(f64),
    Volume(f64),
    Capabilities,
}
struct Envelope {
    page_epoch: u64,
    control: Control,
    reply: tokio::sync::oneshot::Sender<Result<Snapshot, String>>,
}
#[derive(Default)]
pub struct NativeAudioService {
    worker: Mutex<Option<(SyncSender<Envelope>, JoinHandle<()>)>>,
    snapshot: Arc<Mutex<Snapshot>>,
    stopping: Arc<AtomicBool>,
    page_epoch: Arc<AtomicU64>,
    protection: Arc<Mutex<Option<(u64, PathBuf)>>>,
}
impl NativeAudioService {
    pub fn page_loading(&self) {
        self.page_epoch.fetch_add(1, Ordering::AcqRel);
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
                let page_epoch = self.page_epoch.clone();
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
                                page_epoch,
                                protection,
                            )
                        }));
                        if result.is_err() && !stopping.load(Ordering::Acquire) {
                            let mut snapshot = shared.lock().unwrap_or_else(|p| p.into_inner());
                            fail(&mut snapshot, "interrupted");
                            snapshot.revision += 1;
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
                    page_epoch: self.page_epoch.load(Ordering::Acquire),
                })
                .map_err(|_| "audio command queue unavailable")?;
        }
        receive.await.map_err(|_| "interrupted".to_string())?
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
fn fail(snapshot: &mut Snapshot, code: &str) {
    tracing::warn!(
        engine = "native-rust",
        track_id = snapshot.track_id,
        code,
        "audio playback failed"
    );
    snapshot.seeking = false;
    snapshot.status = "error";
    snapshot.initial_loading = false;
    snapshot.error = Some(AudioError {
        code: code.into(),
        message: "Native audio playback failed. Try playing the track again.".into(),
        recoverable: true,
    });
}
struct Pipeline {
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
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        self.output.clock.playing.store(false, Ordering::Release);
        self.active.store(false, Ordering::Release);
        self.seek.cancel_read();
        self.availability.scan_seek.cancel_read();
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
    let decoder_seek = seek.clone();
    let decoder_availability = availability.clone();
    let decoder = std::thread::Builder::new()
        .name("native-audio-decode".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let source = source::Source::open(
                    app,
                    track,
                    generation,
                    token.clone(),
                    decoder_seek.clone(),
                    decoder_availability.clone(),
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
        availability,
    })
}
fn run(
    app: AppHandle,
    rx: Receiver<Envelope>,
    shared: Arc<Mutex<Snapshot>>,
    stopping: Arc<AtomicBool>,
    page_epoch: Arc<AtomicU64>,
    protection: Arc<Mutex<Option<(u64, PathBuf)>>>,
) {
    let mut snapshot = Snapshot::default();
    let mut request: Option<Request> = None;
    let mut current: Option<Pipeline> = None;
    let mut generation = 0;
    let mut desired = false;
    let mut ended = false;
    let mut last_emit = Instant::now();
    let mut last_consumed = 0;
    let mut stalled_since = Instant::now();
    let mut current_page = 0;
    while !stopping.load(Ordering::Acquire) {
        let mut envelope = rx.recv_timeout(Duration::from_millis(20)).ok();
        let page = page_epoch.load(Ordering::Acquire);
        if page != current_page {
            current_page = page;
            // A WebView reload destroys the queue owner. Drop old commands and audio
            // even when JavaScript cleanup never runs.
            current = None;
            request = None;
            desired = false;
            ended = false;
            snapshot = Snapshot {
                revision: snapshot.revision + 1,
                volume_percent: snapshot.volume_percent,
                ..Default::default()
            };
            *shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
            *protection.lock().unwrap_or_else(|p| p.into_inner()) = None;
        }
        if envelope
            .as_ref()
            .is_some_and(|e| e.page_epoch != current_page)
        {
            envelope = None;
        }
        let mut dirty = envelope.is_some();
        if let Some(envelope) = envelope {
            let mut restart = None;
            match envelope.control {
                Control::Capabilities => {
                    let result = if current.is_some() {
                        Ok(snapshot.clone())
                    } else {
                        output::Output::open(snapshot.volume_percent)
                            .map(|_| snapshot.clone())
                            .map_err(str::to_string)
                    };
                    let _ = envelope.reply.send(result);
                    continue;
                }
                Control::Load(next) => {
                    desired = false;
                    ended = false;
                    snapshot.track_id = Some(next.track_id);
                    snapshot.attempt_id = Some(next.attempt_id.clone());
                    snapshot.duration_seconds = next
                        .expected_duration_seconds
                        .filter(|s| s.is_finite() && *s > 0.0)
                        .unwrap_or(0.0);
                    snapshot.current_time_seconds = 0.0;
                    snapshot.seeking = false;
                    request = Some(next);
                    restart = Some(0.0);
                }
                Control::Unload => {
                    current = None;
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
                    if request.is_some() {
                        desired = true;
                        if snapshot.status == "error" || current.is_none() {
                            restart = Some(snapshot.current_time_seconds);
                        }
                        snapshot.status = "buffering";
                    }
                }
                Control::Pause => {
                    desired = false;
                    if let Some(p) = current.as_ref() {
                        p.output.clock.playing.store(false, Ordering::Release);
                    }
                    if matches!(snapshot.status, "loading" | "buffering") {
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
            if let Some(origin) = restart {
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
                        fail(&mut snapshot, code);
                        desired = false;
                    }
                }
            }
            snapshot.revision += 1;
            *shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
            let _ = app.emit("audio:state", snapshot.clone());
            let _ = envelope.reply.send(Ok(snapshot.clone()));
        }
        let mut error = None;
        if let Some(p) = current.as_mut() {
            while let Ok(message) = p.messages.try_recv() {
                dirty = true;
                match message {
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
                    decode::Message::Failed(code) => error = Some(code),
                }
            }
            let clock = &p.output.clock;
            if clock.failed.load(Ordering::Acquire) {
                error = Some("output-unavailable");
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
                if desired && ready && snapshot.status != "playing" {
                    clock.playing.store(true, Ordering::Release);
                }
                if desired && presented > 0 && stalled_since.elapsed() < Duration::from_millis(150)
                {
                    if snapshot.status != "playing" {
                        dirty = true;
                    }
                    snapshot.status = "playing";
                    snapshot.initial_loading = false;
                } else if desired && stalled_since.elapsed() >= Duration::from_millis(150) {
                    if snapshot.status != "buffering" {
                        dirty = true;
                    }
                    snapshot.status = "buffering";
                    if !ready {
                        clock.playing.store(false, Ordering::Release);
                    }
                } else if !desired && ready && matches!(snapshot.status, "loading" | "buffering") {
                    snapshot.status = if snapshot.initial_loading {
                        "ready"
                    } else {
                        "paused"
                    };
                    snapshot.initial_loading = false;
                    dirty = true;
                }
                if (desired && presented > 0) || (!p.landed && ready) {
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
                    && desired
                    && !ended
                    && error.is_none()
                {
                    ended = true;
                    desired = false;
                    clock.playing.store(false, Ordering::Release);
                    snapshot.status = "ended";
                    snapshot.initial_loading = false;
                    dirty = true;
                    snapshot.revision += 1;
                    *shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
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
        if let Some(code) = error {
            current = None;
            desired = false;
            fail(&mut snapshot, code);
            dirty = true;
        }
        if dirty || (current.is_some() && last_emit.elapsed() >= Duration::from_millis(150)) {
            snapshot.revision += 1;
            *shared.lock().unwrap_or_else(|p| p.into_inner()) = snapshot.clone();
            let _ = app.emit("audio:state", snapshot.clone());
            last_emit = Instant::now();
        }
    }
    drop(current);
    *protection.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;
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
