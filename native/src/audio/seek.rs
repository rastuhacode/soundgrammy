//! Latest-wins seek mailbox. Never accessed by the audio callback.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
#[derive(Default)]
pub struct SeekControl {
    pub revision: AtomicU64,
    pub decoder_revision: AtomicU64,
    pub interruptible: AtomicBool,
    pending: Mutex<Option<(u64, f64)>>,
    read: Mutex<Option<Arc<AtomicBool>>>,
}
impl SeekControl {
    pub fn request(&self, seconds: f64) -> u64 {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        *pending = Some((revision, seconds));
        if self.interruptible.load(Ordering::Acquire) {
            self.cancel_read();
        }
        revision
    }
    pub fn take(&self) -> Option<(u64, f64)> {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
    pub fn cancel_read(&self) {
        if let Some(read) = self.read.lock().unwrap_or_else(|e| e.into_inner()).take() {
            read.store(false, Ordering::Release);
        }
    }
    pub fn begin_read(&self) -> Arc<AtomicBool> {
        let token = Arc::new(AtomicBool::new(true));
        *self.read.lock().unwrap_or_else(|e| e.into_inner()) = Some(token.clone());
        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latest_seek_cancels_obsolete_range_waits_and_coalesces_targets() {
        let control = SeekControl::default();
        control.interruptible.store(true, Ordering::Release);
        let read = control.begin_read();
        control.request(10.0);
        control.request(80.0);
        assert!(!read.load(Ordering::Acquire));
        assert_eq!(control.take(), Some((2, 80.0)));
        assert_eq!(control.take(), None);
    }
}
