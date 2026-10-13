//! Session-aware chunk scheduling, Telegram transfer, and sparse-file writes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::{
    stream::{FuturesUnordered, StreamExt},
    FutureExt,
};
use tauri::Manager;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, Notify};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::download;
use crate::telegram::media_requests::{acquire_cancellable, MediaPriority};

use super::progress::ChunkStatus;
use super::{TrackStream, CHUNK_SIZE, FOREGROUND_CONCURRENCY};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ChunkWaitOutcome {
    Ready,
    Retry,
}

pub(super) fn completed_chunk_wait_outcome(status: ChunkStatus) -> Option<ChunkWaitOutcome> {
    match status {
        ChunkStatus::Ready => Some(ChunkWaitOutcome::Ready),
        ChunkStatus::Missing => Some(ChunkWaitOutcome::Retry),
        ChunkStatus::Loading => None,
    }
}

impl TrackStream {
    pub async fn download_complete(self: &Arc<Self>) -> AppResult<PathBuf> {
        self.download_complete_with_active(None).await
    }

    async fn download_complete_with_active(
        self: &Arc<Self>,
        active: Option<Arc<AtomicBool>>,
    ) -> AppResult<PathBuf> {
        let chunk_count = self.state.lock().await.chunks.len();
        let mut next_index = 0usize;
        let mut requests = FuturesUnordered::new();
        let make_request = |index| {
            let stream = Arc::clone(self);
            let request_active = active.clone();
            async move {
                let priority = range_priority(request_active.is_some());
                stream.ensure_chunk(index, request_active, priority).await
            }
        };
        while next_index < chunk_count && requests.len() < FOREGROUND_CONCURRENCY {
            requests.push(make_request(next_index));
            next_index += 1;
        }
        let mut first_error = None;
        while let Some(result) = requests.next().await {
            if let Err(error) = result {
                first_error.get_or_insert(error);
            }
            if first_error.is_none() && next_index < chunk_count {
                requests.push(make_request(next_index));
                next_index += 1;
            }
        }
        if let Some(error) = first_error {
            self.completion.notify_waiters();
            return Err(error);
        }
        self.wait_complete().await
    }

    pub async fn ensure_range(
        self: &Arc<Self>,
        start: u64,
        end: u64,
        active: Option<Arc<AtomicBool>>,
    ) -> AppResult<()> {
        if start > end || end >= self.total {
            return Err(AppError::msg("requested audio range is invalid"));
        }
        if self.destination.exists() {
            return Ok(());
        }

        let first = (start / CHUNK_SIZE) as usize;
        let last = (end / CHUNK_SIZE) as usize;
        let mut requests = FuturesUnordered::new();
        for index in first..=last {
            let stream = Arc::clone(self);
            let active = active.clone();
            requests.push(async move {
                let priority = range_priority(active.is_some());
                stream.ensure_chunk(index, active, priority).await
            });
        }
        while let Some(result) = requests.next().await {
            result?;
        }
        Ok(())
    }

    /// Artwork shares the ledger but fetches missing chunks sequentially at
    /// background priority, independently of its cancellation token.
    pub(super) async fn ensure_artwork_range(
        self: &Arc<Self>,
        start: u64,
        end: u64,
        active: Arc<AtomicBool>,
    ) -> AppResult<()> {
        if start > end || end >= self.total {
            return Err(AppError::msg("requested artwork range is invalid"));
        }
        Self::require_active(Some(&active))?;
        if self.destination.exists() {
            return Ok(());
        }
        for index in (start / CHUNK_SIZE) as usize..=(end / CHUNK_SIZE) as usize {
            self.ensure_chunk(index, Some(active.clone()), MediaPriority::Background)
                .await?;
        }
        Self::require_active(Some(&active))
    }

    async fn ensure_chunk(
        self: &Arc<Self>,
        index: usize,
        active: Option<Arc<AtomicBool>>,
        priority: MediaPriority,
    ) -> AppResult<()> {
        loop {
            Self::require_active(active.as_deref())?;
            let (notify, should_fetch) = {
                let mut state = self.state.lock().await;
                let slot = state
                    .chunks
                    .get_mut(index)
                    .ok_or_else(|| AppError::msg("audio chunk is out of bounds"))?;
                match slot.status {
                    ChunkStatus::Ready => return Ok(()),
                    ChunkStatus::Loading => (Arc::clone(&slot.notify), false),
                    ChunkStatus::Missing => {
                        slot.status = ChunkStatus::Loading;
                        slot.error = None;
                        (Arc::clone(&slot.notify), true)
                    }
                }
            };

            if !should_fetch {
                match self
                    .wait_for_chunk(index, &notify, active.as_deref())
                    .await?
                {
                    ChunkWaitOutcome::Ready => return Ok(()),
                    ChunkWaitOutcome::Retry => continue,
                }
            }

            // The task owns the claim even if its caller drops a range future.
            let stream = Arc::clone(self);
            return tokio::spawn(async move {
                let result = settle_claim(
                    &stream.state,
                    index,
                    stream.fetch_and_store(index, active, priority),
                )
                .await;
                let metadata_result = if result.is_ok() {
                    stream.persist_partial_metadata().await
                } else {
                    Ok(())
                };
                stream.emit_progress().await;
                match result {
                    Ok(_) => {
                        stream.try_finalize().await?;
                        if stream.final_path.lock().await.is_none() {
                            metadata_result?;
                        }
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            })
            .await
            .map_err(|_| AppError::msg("Audio transfer worker interrupted"))?;
        }
    }

    pub(super) fn require_active(active: Option<&AtomicBool>) -> AppResult<()> {
        if active.is_some_and(|active| !active.load(Ordering::Acquire)) {
            return Err(AppError::msg("playback stream session closed"));
        }
        Ok(())
    }

    async fn wait_for_chunk(
        &self,
        index: usize,
        notify: &Notify,
        active: Option<&AtomicBool>,
    ) -> AppResult<ChunkWaitOutcome> {
        let mut notified = Box::pin(notify.notified());
        loop {
            Self::require_active(active)?;
            notified.as_mut().enable();
            {
                let state = self.state.lock().await;
                let slot = &state.chunks[index];
                if let Some(outcome) = completed_chunk_wait_outcome(slot.status) {
                    return Ok(outcome);
                }
            }
            tokio::select! {
                _ = notified.as_mut() => {},
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {},
            }
            notified.set(notify.notified());
        }
    }

    async fn fetch_and_store(
        &self,
        index: usize,
        active: Option<Arc<AtomicBool>>,
        priority: MediaPriority,
    ) -> AppResult<u64> {
        let _permit = acquire_cancellable(&self.request_slots, active.as_deref()).await?;
        Self::require_active(active.as_deref())?;

        let owner = self.owner.lock().await;
        let (track, generation) = owner.clone();
        let document = self.document.lock().await.clone();
        drop(owner);
        let state = self.app.state::<AppState>();
        if state.db.active_uid()? != track.tg_user_id
            || state.db.account_generation()? != generation
        {
            return Err(AppError::NotAuthorized);
        }
        let client = state.ensure_client().await?;
        let first_attempt = download::download_chunk(
            &client,
            &state.media_requests,
            &document,
            index,
            active.as_deref(),
            priority,
        )
        .await;
        let bytes = match first_attempt {
            Ok(bytes) => bytes,
            Err(error) if download::is_file_reference_error(&error) => {
                let _refresh_guard = self.refresh_lock.lock().await;
                if state.db.active_uid()? != track.tg_user_id
                    || state.db.account_generation()? != generation
                {
                    return Err(AppError::NotAuthorized);
                }
                let latest = self.document.lock().await.clone();
                let client = state.client().await?;
                match download::download_chunk(
                    &client,
                    &state.media_requests,
                    &latest,
                    index,
                    active.as_deref(),
                    priority,
                )
                .await
                {
                    Ok(bytes) => bytes,
                    Err(retry_error) if download::is_file_reference_error(&retry_error) => {
                        let refreshed = {
                            let state = self.app.state::<AppState>();
                            download::refresh_file_reference(&state, &track).await?
                        };
                        let owner = self.owner.lock().await;
                        if owner.1 != generation || owner.0.id != track.id {
                            return Err(AppError::NotAuthorized);
                        }
                        *self.document.lock().await = refreshed.clone();
                        drop(owner);
                        let client = state.client().await?;
                        download::download_chunk(
                            &client,
                            &state.media_requests,
                            &refreshed,
                            index,
                            active.as_deref(),
                            priority,
                        )
                        .await?
                    }
                    Err(retry_error) => return Err(retry_error.into()),
                }
            }
            Err(error) => return Err(error.into()),
        };

        let offset = index as u64 * CHUNK_SIZE;
        let expected = (self.total - offset).min(CHUNK_SIZE);
        if bytes.len() as u64 != expected {
            return Err(AppError::msg(format!(
                "Telegram returned {} bytes for chunk {index}, expected {expected}",
                bytes.len()
            )));
        }

        let _write_guard = self.write_lock.lock().await;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&self.partial)
            .await?;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;
        file.sync_data().await?;
        Ok(bytes.len() as u64)
    }
}

async fn settle_claim(
    state: &Mutex<super::progress::StreamState>,
    index: usize,
    transfer: impl std::future::Future<Output = AppResult<u64>>,
) -> AppResult<u64> {
    let result = std::panic::AssertUnwindSafe(transfer)
        .catch_unwind()
        .await
        .unwrap_or_else(|_| Err(AppError::msg("Audio transfer worker panicked")));
    let mut ledger = state.lock().await;
    if let Ok(bytes) = result.as_ref() {
        ledger.received += bytes;
    }
    let slot = &mut ledger.chunks[index];
    slot.status = if result.is_ok() {
        ChunkStatus::Ready
    } else {
        ChunkStatus::Missing
    };
    slot.error = result.as_ref().err().map(ToString::to_string);
    slot.notify.notify_waiters();
    result
}

fn range_priority(cancellable: bool) -> MediaPriority {
    if cancellable {
        MediaPriority::Playback
    } else {
        MediaPriority::Background
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::super::progress::{ChunkSlot, StreamState};
    use super::*;
    #[tokio::test]
    async fn abandoned_range_waiter_does_not_abandon_shared_claim() {
        for fails in [false, true] {
            let state = Arc::new(Mutex::new(StreamState {
                chunks: vec![ChunkSlot::default()],
                received: 0,
                terminal_error: None,
            }));
            state.lock().await.chunks[0].status = ChunkStatus::Loading;
            let (send, receive) = tokio::sync::oneshot::channel();
            let ledger = state.clone();
            let owned = tokio::spawn(async move {
                settle_claim(&ledger, 0, async {
                    receive.await.unwrap();
                    if fails {
                        Err(AppError::msg("injected failure"))
                    } else {
                        Ok(12)
                    }
                })
                .await
            });
            let waiter = tokio::spawn(owned);
            tokio::task::yield_now().await;
            waiter.abort();
            send.send(()).unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(1), async {
                loop {
                    if state.lock().await.chunks[0].status != ChunkStatus::Loading {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("claim remained ownerless");
            let ledger = state.lock().await;
            assert_eq!(
                ledger.chunks[0].status,
                if fails {
                    ChunkStatus::Missing
                } else {
                    ChunkStatus::Ready
                }
            );
            assert_eq!(ledger.received, if fails { 0 } else { 12 });
        }
    }
}
