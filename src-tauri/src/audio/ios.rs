//! CPAL 0.18 owns AudioUnit stop/restart and AVAudioSession reactivation after
//! interruptions. This observer only gates Rust intent; it never restarts a unit.
//! CPAL route removal/reset errors already reach Output::failed and pause safely.
use super::lifecycle::Event;
use block2::RcBlock;
use objc2::{
    rc::Retained,
    runtime::{NSObjectProtocol, ProtocolObject},
};
use objc2_avf_audio::*;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSNumber, NSString};
use std::{
    ptr::NonNull,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, Manager};

pub fn activate() -> Result<(), &'static str> {
    unsafe {
        let session = AVAudioSession::sharedInstance();
        session
            .setCategory_error(AVAudioSessionCategoryPlayback.ok_or("output-unavailable")?)
            .map_err(|_| "output-unavailable")?;
        session
            .setActive_error(true)
            .map_err(|_| "output-unavailable")
    }
}
pub fn deactivate() {
    unsafe {
        let _ = AVAudioSession::sharedInstance().setActive_withOptions_error(
            false,
            AVAudioSessionSetActiveOptions::NotifyOthersOnDeactivation,
        );
    }
}
fn number(notification: &NSNotification, key: Option<&NSString>) -> Option<usize> {
    notification
        .userInfo()?
        .objectForKey(key?)?
        .downcast::<NSNumber>()
        .ok()
        .map(|v| v.unsignedIntegerValue())
}
pub struct Observer(Retained<ProtocolObject<dyn NSObjectProtocol>>);
impl Observer {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app = app.clone();
        let sequence = Arc::new(AtomicU64::new(0));
        let block = RcBlock::new(move |notification: NonNull<NSNotification>| unsafe {
            let n = notification.as_ref();
            let Some(kind) = number(n, AVAudioSessionInterruptionTypeKey) else {
                return;
            };
            let event =
                if AVAudioSessionInterruptionType(kind) == AVAudioSessionInterruptionType::Began {
                    Event::Begin(sequence.fetch_add(1, Ordering::AcqRel) + 1)
                } else {
                    let options = AVAudioSessionInterruptionOptions(
                        number(n, AVAudioSessionInterruptionOptionKey).unwrap_or(0),
                    );
                    Event::End(
                        sequence.load(Ordering::Acquire),
                        options.contains(AVAudioSessionInterruptionOptions::ShouldResume),
                    )
                };
            let _ = app
                .state::<crate::state::AppState>()
                .audio
                .lifecycle_event(event);
        });
        unsafe {
            let name =
                AVAudioSessionInterruptionNotification.ok_or("No interruption notification")?;
            Ok(Self(
                NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    None,
                    &block,
                ),
            ))
        }
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        unsafe {
            NSNotificationCenter::defaultCenter().removeObserver(&self.0);
        }
    }
}
