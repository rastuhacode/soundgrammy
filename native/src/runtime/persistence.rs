//! Coalesced settings writes. UI callers only replace an in-memory snapshot;
//! one worker serializes and writes it without holding the queue lock.
use crate::{
    db::Db,
    error::{AppError, AppResult},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::Duration,
};

type Encode = Box<dyn FnOnce() -> AppResult<String> + Send>;
#[derive(Default)]
struct Pending {
    changes: HashMap<&'static str, Encode>,
    stopping: bool,
}
#[derive(Default)]
struct Queue {
    pending: Mutex<Pending>,
    ready: Condvar,
}
pub(super) struct SettingsWriter {
    queue: Arc<Queue>,
    worker: Option<JoinHandle<()>>,
}
impl SettingsWriter {
    pub(super) fn new(db: Db) -> AppResult<Self> {
        let queue = Arc::new(Queue::default());
        let work = queue.clone();
        let worker = std::thread::Builder::new()
            .name("soundgrammy-settings".into())
            .spawn(move || {
                loop {
                    let mut pending = work.pending.lock().unwrap_or_else(|e| e.into_inner());
                    while pending.changes.is_empty() && !pending.stopping {
                        pending = work.ready.wait(pending).unwrap_or_else(|e| e.into_inner());
                    }
                    // Give rapid input a short window to replace older snapshots.
                    // This is a bounded batching interval, not an indefinitely reset debounce.
                    if !pending.stopping {
                        pending = work
                            .ready
                            .wait_timeout_while(pending, Duration::from_millis(150), |p| {
                                !p.stopping
                            })
                            .unwrap_or_else(|e| e.into_inner())
                            .0;
                    }
                    let changes = std::mem::take(&mut pending.changes);
                    let stopping = pending.stopping;
                    drop(pending);
                    for (key, encode) in changes {
                        if let Err(error) = encode().and_then(|value| db.set_setting(key, &value)) {
                            tracing::error!(%error, key, "Could not persist application settings");
                        }
                    }
                    if stopping {
                        break;
                    }
                }
            })?;
        Ok(Self {
            queue,
            worker: Some(worker),
        })
    }
    pub(super) fn save<T: Serialize + Send + 'static>(
        &self,
        key: &'static str,
        value: T,
    ) -> AppResult<()> {
        let mut pending = self.queue.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending.stopping {
            return Err(AppError::msg("Application closed"));
        }
        pending
            .changes
            .insert(key, Box::new(move || Ok(serde_json::to_string(&value)?)));
        self.queue.ready.notify_one();
        Ok(())
    }
}
impl Drop for SettingsWriter {
    fn drop(&mut self) {
        self.queue
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stopping = true;
        self.queue.ready.notify_one();
        // Shutdown is the only caller that waits for disk: do not lose the last
        // slider value or a diagnostic-history clear when the window closes.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_settings_flush_on_shutdown_and_keep_the_latest_value() {
        let directory = tempfile::tempdir().unwrap();
        let db = Db::open(&directory.path().join("library.db")).unwrap();
        let writer = SettingsWriter::new(db.clone()).unwrap();
        // Contention on the real SQLite connection must not block a UI caller.
        let connection = db.conn.lock().unwrap();
        for volume in 0..1000 {
            writer
                .save("preferences", serde_json::json!({"volume": volume}))
                .unwrap();
        }
        writer.save("logs", vec!["old error"]).unwrap();
        writer.save("logs", Vec::<String>::new()).unwrap();
        drop(connection);
        drop(writer);
        assert_eq!(
            db.get_setting("preferences").unwrap().as_deref(),
            Some("{\"volume\":999}")
        );
        assert_eq!(db.get_setting("logs").unwrap().as_deref(), Some("[]"));
    }
}
