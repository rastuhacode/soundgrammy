//! SMTC is associated with the Tauri HWND; event tokens are removed before release.
use super::{dispatch, Presentation, RemoteCommand};
use tauri::{AppHandle, Manager};
use windows::{
    core::{factory, HSTRING},
    Foundation::{TimeSpan, TypedEventHandler},
    Media::*,
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference},
    Win32::{Foundation::HWND, System::WinRT::ISystemMediaTransportControlsInterop},
};
pub struct Adapter {
    controls: SystemMediaTransportControls,
    button: Option<i64>,
    seek: Option<i64>,
    artwork_path: Option<std::path::PathBuf>,
    artwork: Option<RandomAccessStreamReference>,
}
fn span(seconds: f64) -> TimeSpan {
    TimeSpan {
        Duration: (seconds * 10_000_000.0) as i64,
    }
}
impl Adapter {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let window = app
            .get_webview_window("main")
            .ok_or("Main window unavailable")?;
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let controls: SystemMediaTransportControls = unsafe {
            factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()
                .and_then(|factory| factory.GetForWindow(HWND(hwnd.0)))
        }
        .map_err(|e| e.to_string())?;
        let mut adapter = Self {
            controls,
            button: None,
            seek: None,
            artwork_path: None,
            artwork: None,
        };
        let result = (|| -> windows::core::Result<()> {
            let handle = app.clone();
            adapter.button =
                Some(adapter.controls.ButtonPressed(&TypedEventHandler::<
                    SystemMediaTransportControls,
                    SystemMediaTransportControlsButtonPressedEventArgs,
                >::new(move |_, args| {
                    if let Some(args) = args.as_ref() {
                        let command = match args.Button()? {
                            SystemMediaTransportControlsButton::Play => Some(RemoteCommand::Play),
                            SystemMediaTransportControlsButton::Pause => Some(RemoteCommand::Pause),
                            SystemMediaTransportControlsButton::Stop => Some(RemoteCommand::Stop),
                            SystemMediaTransportControlsButton::Next => Some(RemoteCommand::Next),
                            SystemMediaTransportControlsButton::Previous => {
                                Some(RemoteCommand::Previous)
                            }
                            _ => None,
                        };
                        if let Some(command) = command {
                            dispatch(&handle, command);
                        }
                    }
                    Ok(())
                }))?);
            let handle = app.clone();
            adapter.seek = Some(adapter.controls.PlaybackPositionChangeRequested(
                &TypedEventHandler::<
                    SystemMediaTransportControls,
                    PlaybackPositionChangeRequestedEventArgs,
                >::new(move |_, args| {
                    if let Some(args) = args.as_ref() {
                        dispatch(
                            &handle,
                            RemoteCommand::Seek(
                                args.RequestedPlaybackPosition()?.Duration as f64 / 10_000_000.0,
                            ),
                        );
                    }
                    Ok(())
                }),
            )?);
            adapter.controls.SetIsEnabled(false)?;
            Ok(())
        })();
        result.map_err(|e| e.to_string())?;
        Ok(adapter)
    }
    pub fn update(&mut self, p: &Presentation) -> Result<(), String> {
        self.update_inner(p).map_err(|e| e.to_string())
    }
    fn update_inner(&mut self, p: &Presentation) -> windows::core::Result<()> {
        let c = &self.controls;
        c.SetIsEnabled(p.identity.is_some())?;
        c.SetIsPlayEnabled(p.actions.play)?;
        c.SetIsPauseEnabled(p.actions.pause)?;
        c.SetIsStopEnabled(p.actions.stop)?;
        c.SetIsNextEnabled(p.actions.next)?;
        c.SetIsPreviousEnabled(p.actions.previous)?;
        c.SetPlaybackStatus(match p.status.as_str() {
            "playing" => MediaPlaybackStatus::Playing,
            "loading" | "buffering" => MediaPlaybackStatus::Changing,
            "paused" | "ready" => MediaPlaybackStatus::Paused,
            _ => MediaPlaybackStatus::Stopped,
        })?;
        c.SetPlaybackRate(p.rate)?;
        let display = c.DisplayUpdater()?;
        display.ClearAll()?;
        if let Some(identity) = &p.identity {
            display.SetType(MediaPlaybackType::Music)?;
            display.SetAppMediaId(&HSTRING::from(&identity.attempt))?;
            let music = display.MusicProperties()?;
            music.SetTitle(&HSTRING::from(&p.title))?;
            music.SetArtist(&HSTRING::from(&p.artist))?;
            if self.artwork_path != p.artwork {
                self.artwork_path = p.artwork.clone();
                self.artwork = p.artwork.as_ref().and_then(|path| {
                    let bytes = std::fs::read(path).ok()?;
                    artwork_stream(&bytes).ok()
                });
            }
            if let Some(artwork) = &self.artwork {
                display.SetThumbnail(artwork)?;
            }
        }
        if p.identity.is_none() {
            self.artwork_path = None;
            self.artwork = None;
        }
        display.Update()?;
        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        timeline.SetStartTime(span(0.0))?;
        timeline.SetEndTime(span(p.duration))?;
        timeline.SetMinSeekTime(span(0.0))?;
        timeline.SetMaxSeekTime(span(if p.actions.seek { p.duration } else { 0.0 }))?;
        timeline.SetPosition(span(p.position_now()))?;
        c.UpdateTimelineProperties(&timeline)
    }
}
impl Drop for Adapter {
    fn drop(&mut self) {
        if let Some(token) = self.button.take() {
            let _ = self.controls.RemoveButtonPressed(token);
        }
        if let Some(token) = self.seek.take() {
            let _ = self.controls.RemovePlaybackPositionChangeRequested(token);
        }
        let _ = self.controls.SetIsEnabled(false);
        if let Ok(display) = self.controls.DisplayUpdater() {
            let _ = display.ClearAll();
            let _ = display.Update();
        }
    }
}

// CreateFromUri does not accept file: URLs on Windows. Supply native bytes instead.
fn artwork_stream(bytes: &[u8]) -> windows::core::Result<RandomAccessStreamReference> {
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0)?)?;
    writer.WriteBytes(bytes)?;
    writer.StoreAsync()?.get()?;
    writer.DetachStream()?;
    stream.Seek(0)?;
    RandomAccessStreamReference::CreateFromStream(&stream)
}
