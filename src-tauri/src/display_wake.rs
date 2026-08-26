//! Native user-idle display/system sleep inhibition for fullscreen playback.

use std::sync::{mpsc, Mutex};
use std::thread::{self, JoinHandle};

use crate::error::{AppError, AppResult};

enum WakeRequest {
    Set {
        enabled: bool,
        response: mpsc::SyncSender<Result<(), String>>,
    },
}

/// Owns the worker that holds the fullscreen player's native wake lease.
///
/// Windows execution-state requests are thread-scoped, so creation and drop
/// must happen on the same thread. Using one owner thread also serializes all
/// platforms and makes rapid enable/disable transitions deterministic.
pub struct DisplayWakeState {
    requests: Option<mpsc::Sender<WakeRequest>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl DisplayWakeState {
    pub fn new() -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("display-wake".into())
            .spawn(move || run_worker(receiver))?;
        Ok(Self {
            requests: Some(requests),
            worker: Mutex::new(Some(worker)),
        })
    }

    /// Enables or disables user-idle display/system sleep inhibition.
    ///
    /// `sleep(false)` is intentional: explicit/forced sleep such as closing a
    /// laptop lid, selecting Sleep, or an emergency power event stays under OS
    /// control.
    pub fn set_enabled(&self, enabled: bool) -> AppResult<()> {
        let requests = self
            .requests
            .as_ref()
            .ok_or_else(|| AppError::msg("display wake worker is unavailable"))?;
        let (response, result) = mpsc::sync_channel(1);
        requests
            .send(WakeRequest::Set { enabled, response })
            .map_err(|_| AppError::msg("display wake worker stopped unexpectedly"))?;
        result
            .recv()
            .map_err(|_| AppError::msg("display wake worker stopped unexpectedly"))?
            .map_err(AppError::msg)
    }
}

impl Drop for DisplayWakeState {
    fn drop(&mut self) {
        // Closing the request channel lets the worker drop its native lease on
        // the same thread that acquired it, then exit cleanly.
        self.requests.take();
        if let Ok(worker) = self.worker.get_mut() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

fn run_worker(receiver: mpsc::Receiver<WakeRequest>) {
    let mut lease: Option<keepawake::KeepAwake> = None;
    while let Ok(request) = receiver.recv() {
        match request {
            WakeRequest::Set { enabled, response } => {
                let result = update_lease(&mut lease, enabled);
                let _ = response.send(result);
            }
        }
    }
}

fn update_lease(lease: &mut Option<keepawake::KeepAwake>, enabled: bool) -> Result<(), String> {
    match (enabled, lease.is_some()) {
        (true, false) => {
            let next = keepawake::Builder::default()
                .display(true)
                .idle(true)
                .sleep(false)
                .reason("Fullscreen player is open")
                .app_name("SoundGrammy")
                .app_reverse_domain("com.soundgrammy.app")
                .create()
                .map_err(|error| format!("failed to keep the display awake: {error}"))?;
            *lease = Some(next);
        }
        (false, true) => {
            lease.take();
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabling_an_inactive_lease_is_idempotent() {
        let state = DisplayWakeState::new().unwrap();
        state.set_enabled(false).unwrap();
        state.set_enabled(false).unwrap();
    }
}
