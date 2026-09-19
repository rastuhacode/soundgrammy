//! MediaPlayer objects are created, updated, and released only on Tauri's main thread.
use super::{dispatch, Presentation, RemoteCommand};
use block2::RcBlock;
use objc2::{rc::Retained, runtime::AnyObject, AnyThread};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSImage;
use objc2_foundation::{NSDictionary, NSNumber, NSString};
use objc2_media_player::*;
#[cfg(target_os = "ios")]
use objc2_ui_kit::UIImage as NSImage;
use std::ptr::NonNull;
use tauri::AppHandle;

pub struct Adapter {
    #[cfg(target_os = "ios")]
    _observer: super::super::ios::Observer,
    center: Retained<MPNowPlayingInfoCenter>,
    targets: Vec<(Retained<MPRemoteCommand>, Retained<AnyObject>)>,
    artwork_path: Option<std::path::PathBuf>,
    artwork: Option<Retained<MPMediaItemArtwork>>,
}
impl Adapter {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        // SAFETY: setup and all adapter access run on the main thread; the command
        // center copies blocks, and tokens are retained until explicitly removed.
        unsafe {
            let commands = MPRemoteCommandCenter::sharedCommandCenter();
            let mut adapter = Self {
                #[cfg(target_os = "ios")]
                _observer: super::super::ios::Observer::new(app)?,
                center: MPNowPlayingInfoCenter::defaultCenter(),
                targets: vec![],
                artwork_path: None,
                artwork: None,
            };
            for (command, event) in [
                (commands.playCommand(), RemoteCommand::Play),
                (commands.pauseCommand(), RemoteCommand::Pause),
                (commands.togglePlayPauseCommand(), RemoteCommand::Toggle),
                (commands.stopCommand(), RemoteCommand::Stop),
                (commands.nextTrackCommand(), RemoteCommand::Next),
                (commands.previousTrackCommand(), RemoteCommand::Previous),
            ] {
                let app = app.clone();
                let block = RcBlock::new(move |_: NonNull<MPRemoteCommandEvent>| {
                    if dispatch(&app, event.clone()) {
                        MPRemoteCommandHandlerStatus::Success
                    } else {
                        MPRemoteCommandHandlerStatus::CommandFailed
                    }
                });
                command.setEnabled(false);
                let token = command.addTargetWithHandler(&block);
                adapter.targets.push((command, token));
            }
            let command = commands.changePlaybackPositionCommand();
            let handle = app.clone();
            let block = RcBlock::new(move |event: NonNull<MPRemoteCommandEvent>| {
                // This handler is registered only for changePlaybackPositionCommand.
                let event = &*event
                    .as_ptr()
                    .cast::<MPChangePlaybackPositionCommandEvent>();
                if dispatch(&handle, RemoteCommand::Seek(event.positionTime())) {
                    MPRemoteCommandHandlerStatus::Success
                } else {
                    MPRemoteCommandHandlerStatus::CommandFailed
                }
            });
            command.setEnabled(false);
            let token = command.addTargetWithHandler(&block);
            adapter
                .targets
                .push((Retained::cast_unchecked(command), token));
            Ok(adapter)
        }
    }
    pub fn update(&mut self, p: &Presentation) -> Result<(), String> {
        unsafe {
            for ((command, _), enabled) in self.targets.iter().zip([
                p.actions.play,
                p.actions.pause,
                p.identity.is_some(),
                p.actions.stop,
                p.actions.next,
                p.actions.previous,
                p.actions.seek,
            ]) {
                command.setEnabled(enabled);
            }
            if p.identity.is_none() {
                self.center.setNowPlayingInfo(None);
                #[cfg(target_os = "macos")]
                self.center
                    .setPlaybackState(MPNowPlayingPlaybackState::Stopped);
                self.artwork = None;
                self.artwork_path = None;
                return Ok(());
            }
            if self.artwork_path != p.artwork {
                self.artwork_path = p.artwork.clone();
                self.artwork = p.artwork.as_ref().and_then(|path| {
                    let image = NSImage::initWithContentsOfFile(
                        NSImage::alloc(),
                        &NSString::from_str(&path.to_string_lossy()),
                    )?;
                    let size = image.size();
                    let block = RcBlock::new(move |_| NonNull::from(&*image));
                    Some(MPMediaItemArtwork::initWithBoundsSize_requestHandler(
                        MPMediaItemArtwork::alloc(),
                        size,
                        &block,
                    ))
                });
            }
            let title = NSString::from_str(&p.title);
            let artist = NSString::from_str(&p.artist);
            let duration = NSNumber::new_f64(p.duration);
            let position = NSNumber::new_f64(p.position_now());
            let rate = NSNumber::new_f64(p.rate);
            let identity = NSString::from_str(&p.identity.as_ref().unwrap().attempt);
            let mut keys = vec![
                MPMediaItemPropertyTitle,
                MPMediaItemPropertyArtist,
                MPMediaItemPropertyPlaybackDuration,
                MPNowPlayingInfoPropertyElapsedPlaybackTime,
                MPNowPlayingInfoPropertyPlaybackRate,
                MPNowPlayingInfoPropertyExternalContentIdentifier,
            ];
            let mut values: Vec<&AnyObject> =
                vec![&title, &artist, &duration, &position, &rate, &identity];
            if let Some(artwork) = &self.artwork {
                keys.push(MPMediaItemPropertyArtwork);
                values.push(artwork);
            }
            self.center
                .setNowPlayingInfo(Some(&NSDictionary::from_slices(&keys, &values)));
            #[cfg(target_os = "macos")]
            self.center.setPlaybackState(match p.status.as_str() {
                "playing" => MPNowPlayingPlaybackState::Playing,
                "loading" | "buffering" => MPNowPlayingPlaybackState::Interrupted,
                "error" | "ended" | "idle" => MPNowPlayingPlaybackState::Stopped,
                _ => MPNowPlayingPlaybackState::Paused,
            });
        }
        Ok(())
    }
}
impl Drop for Adapter {
    fn drop(&mut self) {
        unsafe {
            for (command, token) in &self.targets {
                command.setEnabled(false);
                command.removeTarget(Some(token));
            }
            self.center.setNowPlayingInfo(None);
            #[cfg(target_os = "macos")]
            self.center
                .setPlaybackState(MPNowPlayingPlaybackState::Stopped);
        }
    }
}
