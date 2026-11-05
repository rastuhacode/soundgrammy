//! One bounded speculative window per playback prefetch cursor.
use super::*;
use tokio::sync::watch;

// Telegram Desktop's reader preloads eight 128 KiB parts beyond reading demand.
const PARTS_AHEAD: u64 = 8;

#[derive(Clone)]
struct Window {
    first: usize,
    last: usize,
    active: Arc<AtomicBool>,
    read_active: Arc<AtomicBool>,
}

pub(super) struct ReadAhead {
    updates: watch::Sender<Option<Window>>,
    current: Mutex<Option<Window>>,
    total: u64,
}

impl ReadAhead {
    pub fn new(stream: Arc<TrackStream>, active: Arc<AtomicBool>) -> Self {
        let total = stream.total();
        Self::with_fetch(total, active, move |index, active| {
            let stream = stream.clone();
            async move { stream.prefetch_chunk(index, active).await }
        })
    }

    fn with_fetch<F, Fut>(total: u64, active: Arc<AtomicBool>, fetch: F) -> Self
    where
        F: Fn(usize, Arc<AtomicBool>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = crate::error::AppResult<()>> + Send,
    {
        let (updates, mut receiver) = watch::channel::<Option<Window>>(None);
        tauri::async_runtime::spawn(async move {
            'windows: loop {
                let window = receiver.borrow_and_update().clone();
                if let Some(window) = window {
                    for index in window.first..=window.last {
                        if !active.load(Ordering::Acquire)
                            || !window.read_active.load(Ordering::Acquire)
                            || !window.active.load(Ordering::Acquire)
                        {
                            window.active.store(false, Ordering::Release);
                            break;
                        }
                        // One speculative claim at a time: never occupy every
                        // per-stream slot while waiting for background capacity.
                        let transfer = fetch(index, window.active.clone());
                        tokio::pin!(transfer);
                        loop {
                            tokio::select! {
                                result = &mut transfer => {
                                    if result.is_err() {
                                        window.active.store(false, Ordering::Release);
                                    }
                                    break;
                                }
                                changed = receiver.changed() => {
                                    let retained = receiver.borrow().as_ref().is_some_and(|next| {
                                        Arc::ptr_eq(&next.active, &window.active)
                                    });
                                    if !retained { window.active.store(false, Ordering::Release); }
                                    if changed.is_err() { return; }
                                    continue 'windows;
                                }
                                _ = tokio::time::sleep(Duration::from_millis(25)) => {
                                    if !active.load(Ordering::Acquire)
                                        || !window.read_active.load(Ordering::Acquire)
                                    {
                                        window.active.store(false, Ordering::Release);
                                        break;
                                    }
                                }
                            }
                        }
                        if !window.active.load(Ordering::Acquire) {
                            break;
                        }
                    }
                }
                if receiver.changed().await.is_err() {
                    return;
                }
            }
        });
        Self {
            updates,
            current: Mutex::new(None),
            total,
        }
    }

    pub fn update(&self, end: u64, read_active: Arc<AtomicBool>) {
        let Some((first, last)) = window_after(end, self.total) else {
            self.cancel();
            return;
        };
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        let active = if let Some(previous) = current
            .as_ref()
            .filter(|window| first == window.first + 1 && window.active.load(Ordering::Acquire))
        {
            // An in-flight next part becomes demanded on sequential advance.
            // Keep its claim alive rather than cancel and redownload it.
            previous.active.clone()
        } else {
            if let Some(previous) = current.as_ref() {
                previous.active.store(false, Ordering::Release);
            }
            Arc::new(AtomicBool::new(true))
        };
        let window = Window {
            first,
            last,
            active,
            read_active,
        };
        *current = Some(window.clone());
        self.updates.send_replace(Some(window));
    }

    pub fn cancel(&self) {
        if let Some(window) = self
            .current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            window.active.store(false, Ordering::Release);
        }
        self.updates.send_replace(None);
    }
}

impl Drop for ReadAhead {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn window_after(end: u64, total: u64) -> Option<(usize, usize)> {
    let first = end / CHUNK_SIZE + 1;
    let count = total.div_ceil(CHUNK_SIZE);
    (first < count).then(|| {
        (
            first as usize,
            (first + PARTS_AHEAD - 1).min(count - 1) as usize,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rolling_window_is_eight_parts_and_clamps_at_eof() {
        assert_eq!(window_after(CHUNK_SIZE - 1, CHUNK_SIZE * 20), Some((1, 8)));
        assert_eq!(
            window_after(CHUNK_SIZE * 7 - 1, CHUNK_SIZE * 20),
            Some((7, 14))
        );
        assert_eq!(
            window_after(CHUNK_SIZE * 19 - 1, CHUNK_SIZE * 20 + 17),
            Some((19, 20))
        );
        assert_eq!(
            window_after(CHUNK_SIZE * 20 + 16, CHUNK_SIZE * 20 + 17),
            None
        );
        assert_eq!(window_after(16, 17), None);
    }

    #[tokio::test]
    async fn sequential_advance_keeps_the_now_demanded_part_alive() {
        let (events, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let ahead = ReadAhead::with_fetch(
            CHUNK_SIZE * 100,
            Arc::new(AtomicBool::new(true)),
            move |index, active| {
                events.send((index, active)).unwrap();
                std::future::pending()
            },
        );
        ahead.update(CHUNK_SIZE - 1, Arc::new(AtomicBool::new(true)));
        let (_, first) = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        ahead.update(CHUNK_SIZE * 2 - 1, Arc::new(AtomicBool::new(true)));
        let (index, next) = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(index, 2);
        assert!(
            first.load(Ordering::Acquire),
            "sequential demand cancelled its in-flight part"
        );
        assert!(Arc::ptr_eq(&first, &next));
        ahead.cancel();
        assert!(!first.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn seek_replaces_a_blocked_window_and_drop_cancels_its_claim() {
        let (events, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let session = Arc::new(AtomicBool::new(true));
        let ahead = ReadAhead::with_fetch(CHUNK_SIZE * 100, session, move |index, active| {
            events.send((index, active)).unwrap();
            std::future::pending()
        });
        ahead.update(CHUNK_SIZE - 1, Arc::new(AtomicBool::new(true)));
        let (first, old) = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first, 1);
        ahead.update(CHUNK_SIZE * 71 - 1, Arc::new(AtomicBool::new(true)));
        let (target, current) = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            target, 71,
            "seek must bypass the intervening speculative queue"
        );
        assert!(!old.load(Ordering::Acquire));
        drop(ahead);
        assert!(!current.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn closed_playback_and_cancelled_reads_stop_new_speculation() {
        for cancel_session in [false, true] {
            let (events, mut requests) = tokio::sync::mpsc::unbounded_channel();
            let session = Arc::new(AtomicBool::new(true));
            let read = Arc::new(AtomicBool::new(true));
            let ahead =
                ReadAhead::with_fetch(CHUNK_SIZE * 100, session.clone(), move |index, active| {
                    events.send((index, active)).unwrap();
                    std::future::pending()
                });
            ahead.update(CHUNK_SIZE - 1, read.clone());
            let (_, claim) = tokio::time::timeout(Duration::from_secs(2), requests.recv())
                .await
                .unwrap()
                .unwrap();
            if cancel_session {
                session.store(false, Ordering::Release);
            } else {
                read.store(false, Ordering::Release);
            }
            tokio::time::timeout(Duration::from_secs(2), async {
                while claim.load(Ordering::Acquire) {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert!(
                requests.try_recv().is_err(),
                "cancelled window scheduled another part"
            );
        }
    }

    #[tokio::test]
    async fn a_completed_window_does_not_download_the_rest_of_the_track() {
        let (events, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let ahead = ReadAhead::with_fetch(
            CHUNK_SIZE * 100,
            Arc::new(AtomicBool::new(true)),
            move |index, _| {
                events.send(index).unwrap();
                std::future::ready(Ok(()))
            },
        );
        ahead.update(CHUNK_SIZE - 1, Arc::new(AtomicBool::new(true)));
        for expected in 1..=8 {
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), requests.recv())
                    .await
                    .unwrap(),
                Some(expected)
            );
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(50), requests.recv())
                .await
                .is_err()
        );
    }
}
