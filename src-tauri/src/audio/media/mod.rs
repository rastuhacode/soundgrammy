//! Presentation-only bridge. The audio worker remains the sole transport/queue owner.
use super::{session, Control, Snapshot};
use serde::Serialize;
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[path = "apple.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(target_os = "android")]
#[path = "android.rs"]
pub(super) mod platform;
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux",
    target_os = "android"
)))]
mod platform {
    pub struct Adapter;
    impl Adapter {
        pub fn new(_: &tauri::AppHandle) -> Result<Self, String> {
            Ok(Self)
        }
        pub fn update(&mut self, _: &super::Presentation) -> Result<(), String> {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Identity {
    pub track: i64,
    pub attempt: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Actions {
    pub play: bool,
    pub pause: bool,
    pub stop: bool,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Presentation {
    pub revision: u64,
    pub identity: Option<Identity>,
    pub title: String,
    pub artist: String,
    pub artwork: Option<PathBuf>,
    pub status: String,
    /// Includes the natural-end handoff while the native queue chooses its successor.
    pub background_active: bool,
    pub duration: f64,
    pub position: f64,
    pub rate: f64,
    pub actions: Actions,
    #[serde(skip)]
    pub sampled_at: Instant,
}
impl Presentation {
    fn from_snapshot(s: &Snapshot) -> Self {
        let track = s.player.as_ref().and_then(|p| p.queue.current());
        let active = track.is_some() && s.track_id == track.map(|t| t.id);
        let desired = s.player.as_ref().is_some_and(|p| p.is_playing)
            && !matches!(s.status, "error" | "ended" | "idle");
        let previous = s.player.as_ref().is_some_and(|p| {
            p.queue.cursor > 0
                || p.preferences.repeat != session::Repeat::None
                || s.current_time_seconds >= 5.0
        });
        Self {
            revision: s.revision,
            identity: if active {
                s.attempt_id.as_ref().map(|a| Identity {
                    track: s.track_id.unwrap(),
                    attempt: a.clone(),
                })
            } else {
                None
            },
            title: track
                .and_then(|t| t.title.clone())
                .unwrap_or_else(|| "Unknown Title".into()),
            artist: track
                .and_then(|t| t.performer.clone())
                .unwrap_or_else(|| "Unknown Artist".into()),
            artwork: None,
            status: s.status.into(),
            background_active: active
                && s.player.as_ref().is_some_and(|p| p.is_playing)
                && !matches!(s.status, "error" | "idle"),
            duration: finite(s.duration_seconds),
            position: finite(s.current_time_seconds),
            rate: if s.status == "playing" { 1.0 } else { 0.0 },
            actions: Actions {
                play: active && !desired,
                pause: active && desired,
                stop: active,
                // Next at the final item intentionally pauses, matching the in-app command.
                next: active,
                previous: active && previous,
                seek: active && finite(s.duration_seconds) > 0.0,
            },
            sampled_at: Instant::now(),
        }
    }
    pub fn position_now(&self) -> f64 {
        (self.position + self.sampled_at.elapsed().as_secs_f64() * self.rate)
            .min(self.duration)
            .max(0.0)
    }
}
fn finite(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // Each adapter supports a different subset; all mappings are tested.
pub enum RemoteCommand {
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    Seek(f64),
    SeekBy(f64),
    SeekFor(Identity, f64),
}

/// An OS command is meaningful only for the playback attempt that was presented
/// when its callback arrived. Natural completion is processed ahead of callbacks,
/// so carrying this identity prevents a late pause/next from affecting its successor.
#[derive(Clone, Debug)]
pub struct QueuedRemote {
    command: RemoteCommand,
    identity: Option<Identity>,
}
impl QueuedRemote {
    pub fn capture(command: RemoteCommand, snapshot: &Snapshot) -> Self {
        let identity = Presentation::from_snapshot(snapshot).identity;
        tracing::debug!(?command, ?identity, "queued native media command");
        Self { command, identity }
    }

    pub(super) fn control(self, snapshot: &Snapshot) -> Option<Control> {
        let current = Presentation::from_snapshot(snapshot).identity;
        if self.identity != current {
            tracing::info!(
                command = ?self.command,
                queued_for = ?self.identity,
                current = ?current,
                "ignored stale native media command"
            );
            return None;
        }
        tracing::debug!(command = ?self.command, identity = ?current, "applying native media command");
        self.command.control(snapshot)
    }
}

impl RemoteCommand {
    pub(super) fn control(self, snapshot: &Snapshot) -> Option<Control> {
        let p = Presentation::from_snapshot(snapshot);
        use RemoteCommand::*;
        Some(match self {
            Play if p.actions.play => session::Command::Playing { playing: true }.into(),
            Pause if p.actions.pause => session::Command::Playing { playing: false }.into(),
            Toggle if p.identity.is_some() => session::Command::Toggle.into(),
            Stop if p.actions.stop => Control::Stop,
            Next if p.actions.next => session::Command::Next.into(),
            Previous if p.actions.previous => session::Command::Previous { restart: true }.into(),
            SeekFor(identity, v)
                if p.identity.as_ref() == Some(&identity) && p.actions.seek && v.is_finite() =>
            {
                Control::Seek(v.clamp(0.0, p.duration))
            }
            Seek(v) if p.actions.seek && v.is_finite() => Control::Seek(v.clamp(0.0, p.duration)),
            SeekBy(v) if p.actions.seek && v.is_finite() => {
                Control::Seek((p.position + v).clamp(0.0, p.duration))
            }
            _ => return None,
        })
    }
}

pub fn dispatch(app: &AppHandle, command: RemoteCommand) -> bool {
    app.state::<crate::state::AppState>()
        .audio
        .remote_command(command)
        .is_ok()
}

#[derive(Default)]
struct State {
    current: Option<Presentation>,
    closed: bool,
    scheduled: bool,
}
impl State {
    fn accept(&mut self, mut next: Presentation, immediate: bool) -> bool {
        if self.closed
            || self
                .current
                .as_ref()
                .is_some_and(|p| p.revision >= next.revision)
        {
            return false;
        }
        let changed_identity =
            self.current.as_ref().and_then(|p| p.identity.as_ref()) != next.identity.as_ref();
        if let Some(old) = self.current.as_ref() {
            if !changed_identity {
                next.artwork = old.artwork.clone();
                let transition = old.status != next.status
                    || old.actions != next.actions
                    || old.duration != next.duration
                    || old.title != next.title
                    || old.artist != next.artist
                    || (old.position_now() - next.position).abs() > 0.5;
                if !immediate && !transition && old.sampled_at.elapsed() < Duration::from_secs(1) {
                    return false;
                }
            }
        }
        self.current = Some(next);
        true
    }
    fn close(&mut self) {
        self.closed = true;
        self.current = None;
    }
    fn artwork(&mut self, identity: &Identity, path: PathBuf) -> bool {
        if self.closed {
            return false;
        }
        match self.current.as_mut() {
            Some(p) if p.identity.as_ref() == Some(identity) => {
                p.artwork = Some(path);
                true
            }
            _ => false,
        }
    }
}
#[derive(Default)]
pub struct Bridge {
    state: Mutex<State>,
}
// The initializer is const; Android's fallback TLS macro still triggers this lint.
thread_local! { #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local))] static ADAPTER: RefCell<Option<platform::Adapter>> = const { RefCell::new(None) }; }

/// Called once on Tauri's main thread. The adapter is never owned by a WebView.
pub fn initialize(app: &AppHandle) {
    app.manage(Bridge::default());
    ADAPTER.with(|adapter| match platform::Adapter::new(app) {
        Ok(value) => *adapter.borrow_mut() = Some(value),
        Err(error) => tracing::warn!(%error, "Native media bridge unavailable"),
    });
}
pub fn publish(app: &AppHandle, snapshot: &Snapshot, immediate: bool) {
    let Some(bridge) = app.try_state::<Bridge>() else {
        return;
    };
    let next = Presentation::from_snapshot(snapshot);
    let mut state = bridge.state.lock().unwrap_or_else(|p| p.into_inner());
    let artwork_identity = (state.current.as_ref().and_then(|p| p.identity.as_ref())
        != next.identity.as_ref())
    .then(|| next.identity.clone())
    .flatten();
    if !state.accept(next, immediate) {
        return;
    }
    drop(state);
    schedule(app);
    if let Some(identity) = artwork_identity {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let result = crate::cache::ensure_thumbnail(
                &app.state::<crate::state::AppState>(),
                identity.track,
                false,
            )
            .await;
            if let Ok(Some(path)) = result {
                let accepted = app
                    .state::<Bridge>()
                    .state
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .artwork(&identity, path);
                if accepted {
                    schedule(&app);
                }
            }
        });
    }
}
fn schedule(app: &AppHandle) {
    let bridge = app.state::<Bridge>();
    let mut state = bridge.state.lock().unwrap_or_else(|p| p.into_inner());
    if state.closed || state.scheduled {
        return;
    }
    state.scheduled = true;
    drop(state);
    let handle = app.clone();
    if app
        .run_on_main_thread(move || {
            let bridge = handle.state::<Bridge>();
            let presentation = {
                let mut state = bridge.state.lock().unwrap_or_else(|p| p.into_inner());
                state.scheduled = false;
                if state.closed {
                    return;
                }
                state.current.clone()
            };
            if let Some(p) = presentation {
                ADAPTER.with(|a| {
                    if let Some(a) = a.borrow_mut().as_mut() {
                        if let Err(error) = a.update(&p) {
                            tracing::warn!(%error, "Native media update failed");
                        }
                    }
                });
            }
        })
        .is_err()
    {
        bridge
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .scheduled = false;
    }
}
/// Called on the app thread after stopping audio. Pending art and updates become no-ops.
pub fn shutdown(app: &AppHandle) {
    if let Some(bridge) = app.try_state::<Bridge>() {
        let mut state = bridge.state.lock().unwrap_or_else(|p| p.into_inner());
        state.close();
    }
    ADAPTER.with(|a| {
        a.borrow_mut().take();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn snapshot() -> Snapshot {
        let track = serde_json::from_value(serde_json::json!({"id":7,"tg_user_id":1,"file_id":"","file_unique_id":"","title":"Title","performer":"Artist","duration":120,"source":"saved_music","mime_type":null,"file_size":null,"created_at":""})).unwrap();
        let mut session = session::Session::default();
        session
            .apply(
                session::Command::SetQueue {
                    queue: session::Queue {
                        tracks: vec![track; 2],
                        cursor: 0,
                        ..Default::default()
                    },
                    play: true,
                },
                0.0,
                |e, _| e,
            )
            .unwrap();
        Snapshot {
            revision: 1,
            track_id: Some(7),
            attempt_id: Some(format!("native:{}", session.attempt)),
            status: "playing",
            duration_seconds: 120.0,
            current_time_seconds: 10.0,
            player: Some(Arc::new(session)),
            ..Default::default()
        }
    }
    #[test]
    fn actions_and_rate_follow_transport_and_queue_policy() {
        let mut s = snapshot();
        let p = Presentation::from_snapshot(&s);
        assert_eq!(p.title, "Title");
        assert_eq!(p.artist, "Artist");
        assert_eq!(p.rate, 1.0);
        assert_eq!(
            p.actions,
            Actions {
                play: false,
                pause: true,
                stop: true,
                next: true,
                previous: true,
                seek: true
            }
        );
        s.status = "buffering";
        assert_eq!(Presentation::from_snapshot(&s).rate, 0.0);
        assert!(Presentation::from_snapshot(&s).actions.pause);
        s.status = "error";
        Arc::make_mut(s.player.as_mut().unwrap()).is_playing = false;
        assert!(Presentation::from_snapshot(&s).actions.play);
        s.current_time_seconds = 2.0;
        assert!(!Presentation::from_snapshot(&s).actions.previous);
        Arc::make_mut(s.player.as_mut().unwrap()).preferences.repeat = session::Repeat::All;
        assert!(Presentation::from_snapshot(&s).actions.previous);
        let empty = Presentation::from_snapshot(&Snapshot::default());
        assert_eq!(empty.actions, Actions::default());
        assert!(empty.identity.is_none());
    }

    #[test]
    fn foreground_lifetime_survives_queue_handoff_and_transient_interruption() {
        let mut s = snapshot();
        s.status = "paused"; // OS interruption, user still intends playback.
        assert!(Presentation::from_snapshot(&s).background_active);
        s.status = "ended"; // Next queue item has not been selected yet.
        assert!(Presentation::from_snapshot(&s).background_active);
        Arc::make_mut(s.player.as_mut().unwrap()).is_playing = false;
        assert!(!Presentation::from_snapshot(&s).background_active);
        s.status = "error";
        assert!(!Presentation::from_snapshot(&s).background_active);
    }
    #[test]
    fn pause_queued_for_completed_attempt_cannot_pause_loading_successor() {
        let ending = snapshot();
        let queued = QueuedRemote::capture(RemoteCommand::Pause, &ending);

        let mut successor = ending.clone();
        let session = Arc::make_mut(successor.player.as_mut().unwrap());
        assert_eq!(session.complete(session.attempt), session::Effect::Load);
        successor.track_id = session.queue.current().map(|track| track.id);
        successor.attempt_id = Some(format!("native:{}", session.attempt));
        successor.status = "loading";

        assert!(queued.control(&successor).is_none());
        assert!(matches!(
            QueuedRemote::capture(RemoteCommand::Pause, &successor).control(&successor),
            Some(Control::Player(command))
                if matches!(*command, session::Command::Playing { playing: false })
        ));
    }
    #[test]
    fn native_mapping_reuses_session_semantics_and_duplicate_attempts() {
        let mut s = snapshot();
        assert!(
            matches!(RemoteCommand::Pause.control(&s), Some(Control::Player(c)) if matches!(*c, session::Command::Playing { playing: false }))
        );
        assert!(
            matches!(RemoteCommand::Toggle.control(&s), Some(Control::Player(c)) if matches!(*c, session::Command::Toggle))
        );
        assert!(matches!(
            RemoteCommand::Stop.control(&s),
            Some(Control::Stop)
        ));
        assert!(RemoteCommand::Play.control(&s).is_none());
        let old = Presentation::from_snapshot(&s).identity.unwrap();
        let Some(Control::Player(command)) = RemoteCommand::Previous.control(&s) else {
            panic!()
        };
        let session = Arc::make_mut(s.player.as_mut().unwrap());
        assert!(matches!(
            session.apply(*command, 10.0, |e, _| e).unwrap(),
            session::Effect::Seek(0.0)
        ));
        let Some(Control::Player(command)) = RemoteCommand::Next.control(&s) else {
            panic!()
        };
        let session = Arc::make_mut(s.player.as_mut().unwrap());
        assert!(matches!(
            session.apply(*command, 10.0, |e, _| e).unwrap(),
            session::Effect::Load
        ));
        assert_eq!(session.queue.cursor, 1);
        s.attempt_id = Some(format!("native:{}", session.attempt));
        let new = Presentation::from_snapshot(&s).identity.unwrap();
        assert_eq!(old.track, new.track);
        assert_ne!(old, new);
        assert!(RemoteCommand::SeekFor(old, 3.0).control(&s).is_none());
        assert!(matches!(
            RemoteCommand::SeekFor(new, 3.0).control(&s),
            Some(Control::Seek(3.0))
        ));
    }
    #[test]
    fn seeking_rejects_invalid_values_and_clamps_to_duration() {
        let s = snapshot();
        assert!(RemoteCommand::Seek(f64::NAN).control(&s).is_none());
        assert!(RemoteCommand::SeekBy(f64::INFINITY).control(&s).is_none());
        assert!(matches!(
            RemoteCommand::Seek(999.0).control(&s),
            Some(Control::Seek(120.0))
        ));
        assert!(matches!(
            RemoteCommand::SeekBy(-30.0).control(&s),
            Some(Control::Seek(0.0))
        ));
        assert!(RemoteCommand::Next.control(&Snapshot::default()).is_none());
        assert!(RemoteCommand::Stop.control(&Snapshot::default()).is_none());
    }
    #[test]
    fn artwork_is_bound_to_track_and_attempt_and_cannot_resurrect_teardown() {
        let mut p = Presentation::from_snapshot(&snapshot());
        let old = p.identity.clone().unwrap();
        let mut state = State {
            current: Some(p.clone()),
            ..Default::default()
        };
        assert!(state.artwork(&old, PathBuf::from("first.jpg")));
        p.identity.as_mut().unwrap().attempt = "native:next".into();
        state.current = Some(p.clone());
        assert!(!state.artwork(&old, PathBuf::from("stale.jpg")));
        assert!(state.current.as_ref().unwrap().artwork.is_none());
        state.current = Some(Presentation::from_snapshot(&Snapshot::default()));
        assert!(!state.artwork(
            p.identity.as_ref().unwrap(),
            PathBuf::from("after-clear.jpg")
        ));
        state.current = Some(p.clone());
        state.close();
        assert!(state.current.is_none());
        assert!(!state.artwork(
            p.identity.as_ref().unwrap(),
            PathBuf::from("after-close.jpg")
        ));
    }
}

#[cfg(test)]
mod ordering_tests {
    use super::*;
    #[test]
    fn coalescing_preserves_transitions_seeks_revisions_and_shutdown() {
        let mut state = State::default();
        let mut next = Presentation::from_snapshot(&Snapshot::default());
        next.revision = 10;
        assert!(state.accept(next.clone(), false));
        next.revision = 9;
        assert!(!state.accept(next.clone(), true));
        next.revision = 11;
        assert!(!state.accept(next.clone(), false));
        // Even a sub-second seek is immediate; a repeated snapshot never regresses it.
        next.position = 0.1;
        assert!(state.accept(next.clone(), true));
        assert!(!state.accept(next.clone(), true));
        next.revision = 12;
        next.status = "buffering".into();
        assert!(state.accept(next.clone(), false));
        next.revision = 13;
        state.current.as_mut().unwrap().sampled_at -= Duration::from_secs(2);
        assert!(state.accept(next.clone(), false));
        state.close();
        next.revision = 14;
        assert!(!state.accept(next, true));
        assert!(state.current.is_none());
    }
}
