//! MPRIS on the session bus. zbus owns the D-Bus executor; callbacks enqueue Rust commands.
use super::{dispatch, Presentation, RemoteCommand};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tauri::AppHandle;
use zbus::{
    blocking::{connection::Builder, Connection},
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
const PATH: &str = "/org/mpris/MediaPlayer2";
type Shared = Arc<RwLock<Option<Presentation>>>;
struct Root;
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {}
    fn quit(&self) {}
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "SoundGrammy"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec![]
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        vec![]
    }
}
struct Player {
    app: AppHandle,
    state: Shared,
}
impl Player {
    fn current(&self) -> Option<Presentation> {
        self.state.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    fn send(&self, command: RemoteCommand) {
        dispatch(&self.app, command);
    }
}
fn track_path(p: &Presentation) -> OwnedObjectPath {
    OwnedObjectPath::try_from(
        p.identity
            .as_ref()
            .map(|i| {
                format!(
                    "/org/mpris/MediaPlayer2/track/t{}_{}",
                    i.track.unsigned_abs(),
                    hex::encode(&i.attempt)
                )
            })
            .unwrap_or_else(|| "/org/mpris/MediaPlayer2/TrackList/NoTrack".into()),
    )
    .unwrap()
}
fn metadata(p: Option<&Presentation>) -> HashMap<String, OwnedValue> {
    let mut map = HashMap::new();
    if let Some(p) = p.filter(|p| p.identity.is_some()) {
        map.insert(
            "mpris:trackid".into(),
            Value::from(track_path(p)).try_into().unwrap(),
        );
        map.insert(
            "mpris:length".into(),
            ((p.duration * 1_000_000.0) as i64).into(),
        );
        map.insert(
            "xesam:title".into(),
            Value::from(p.title.clone()).try_into().unwrap(),
        );
        map.insert(
            "xesam:artist".into(),
            Value::from(vec![p.artist.clone()]).try_into().unwrap(),
        );
        if let Some(url) = p
            .artwork
            .as_ref()
            .and_then(|p| url::Url::from_file_path(p).ok())
        {
            map.insert(
                "mpris:artUrl".into(),
                Value::from(url.to_string()).try_into().unwrap(),
            );
        }
    }
    map
}
fn status(p: Option<&Presentation>) -> &'static str {
    match p.map(|p| p.status.as_str()) {
        Some("playing") => "Playing",
        Some("paused" | "ready" | "buffering" | "loading") => "Paused",
        _ => "Stopped",
    }
}
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) {
        self.send(RemoteCommand::Next);
    }
    fn previous(&self) {
        self.send(RemoteCommand::Previous);
    }
    fn pause(&self) {
        self.send(RemoteCommand::Pause);
    }
    fn play_pause(&self) {
        self.send(RemoteCommand::Toggle);
    }
    fn stop(&self) {
        self.send(RemoteCommand::Stop);
    }
    fn play(&self) {
        self.send(RemoteCommand::Play);
    }
    fn seek(&self, offset: i64) {
        self.send(RemoteCommand::SeekBy(offset as f64 / 1_000_000.0));
    }
    fn set_position(&self, track_id: OwnedObjectPath, position: i64) {
        if let Some(p) = self.current().filter(|p| {
            track_path(p) == track_id
                && position >= 0
                && position as f64 <= p.duration * 1_000_000.0
        }) {
            if let Some(identity) = p.identity {
                self.send(RemoteCommand::SeekFor(
                    identity,
                    position as f64 / 1_000_000.0,
                ));
            }
        }
    }
    fn open_uri(&self, _uri: &str) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::NotSupported(
            "Use the SoundGrammy library".into(),
        ))
    }
    #[zbus(property)]
    fn playback_status(&self) -> String {
        status(self.current().as_ref()).into()
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn set_rate(&self, value: f64) -> zbus::fdo::Result<()> {
        if value == 1.0 {
            Ok(())
        } else {
            Err(zbus::fdo::Error::NotSupported("Fixed playback rate".into()))
        }
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        metadata(self.current().as_ref())
    }
    // Volume control is optional to this bridge; advertise a fixed value.
    #[zbus(property)]
    fn volume(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn set_volume(&self, _value: f64) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::NotSupported("Use in-app volume".into()))
    }
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        self.current()
            .map(|p| (p.position_now() * 1_000_000.0) as i64)
            .unwrap_or(0)
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.current().is_some_and(|p| p.actions.next)
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.current().is_some_and(|p| p.actions.previous)
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.current().is_some_and(|p| p.identity.is_some())
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.current().is_some_and(|p| p.identity.is_some())
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.current().is_some_and(|p| p.actions.seek)
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}
pub struct Adapter {
    connection: Connection,
    state: Shared,
}
impl Adapter {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let state = Shared::default();
        let connection = (|| -> zbus::Result<Connection> {
            Builder::session()?
                .name(format!(
                    "org.mpris.MediaPlayer2.SoundGrammy.instance{}",
                    std::process::id()
                ))?
                .serve_at(PATH, Root)?
                .serve_at(
                    PATH,
                    Player {
                        app: app.clone(),
                        state: state.clone(),
                    },
                )?
                .build()
        })()
        .map_err(|e| e.to_string())?;
        Ok(Self { connection, state })
    }
    pub fn update(&mut self, p: &Presentation) -> Result<(), String> {
        let previous = self
            .state
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .replace(p.clone());
        let mut changed: HashMap<&str, OwnedValue> = HashMap::new();
        changed.insert(
            "Metadata",
            Value::from(metadata(Some(p))).try_into().unwrap(),
        );
        changed.insert(
            "PlaybackStatus",
            Value::from(status(Some(p))).try_into().unwrap(),
        );
        for (name, value) in [
            ("CanGoNext", p.actions.next),
            ("CanGoPrevious", p.actions.previous),
            ("CanPlay", p.identity.is_some()),
            ("CanPause", p.identity.is_some()),
            ("CanSeek", p.actions.seek),
        ] {
            changed.insert(name, value.into());
        }
        self.connection
            .emit_signal(
                None::<&str>,
                PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(
                    "org.mpris.MediaPlayer2.Player",
                    changed,
                    Vec::<String>::new(),
                ),
            )
            .map_err(|e| e.to_string())?;
        if previous.is_some_and(|old| {
            old.identity == p.identity && (old.position_now() - p.position).abs() > 0.5
        }) {
            self.connection
                .emit_signal(
                    None::<&str>,
                    PATH,
                    "org.mpris.MediaPlayer2.Player",
                    "Seeked",
                    &((p.position * 1_000_000.0) as i64,),
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
// Dropping the connection unregisters the bus name, interfaces, and all callbacks.
