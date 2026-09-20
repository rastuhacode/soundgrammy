//! Session-aware chunk scheduling, Telegram transfer, and sparse-file writes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::stream::{FuturesUnordered, StreamExt};
use tauri::{Emitter, Manager};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::download;

use super::progress::{progress_from_state, ChunkStatus};
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
            async move { stream.ensure_chunk(index, request_active).await }
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
            requests.push(async move { stream.ensure_chunk(index, active).await });
        }
        while let Some(result) = requests.next().await {
            result?;
        }
        Ok(())
    }

    async fn ensure_chunk(
        self: &Arc<Self>,
        index: usize,
        active: Option<Arc<AtomicBool>>,
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

            Self::require_active(active.as_deref())?;
            let result = self.fetch_and_store(index, active.clone()).await;
            let mut state = self.state.lock().await;
            let notify = Arc::clone(&state.chunks[index].notify);
            if let Ok(bytes_written) = &result {
                state.received += *bytes_written;
            }
            let slot = &mut state.chunks[index];
            match &result {
                Ok(_) => {
                    slot.status = ChunkStatus::Ready;
                    slot.error = None;
                }
                Err(error) => {
                    slot.status = ChunkStatus::Missing;
                    slot.error = Some(error.to_string());
                }
            }
            notify.notify_waiters();
            let progress = progress_from_state(self.track.id, self.total, &state);
            drop(state);
            let metadata_result = if result.is_ok() {
                self.persist_partial_metadata().await
            } else {
                Ok(())
            };
            let _ = self.app.emit("download:progress", progress);

            return match result {
                Ok(_) => {
                    self.try_finalize().await?;
                    if self.final_path.lock().await.is_none() {
                        metadata_result?;
                    }
                    Ok(())
                }
                Err(error) => Err(error),
            };
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
            notified.as_mut().await;
            notified.set(notify.notified());
        }
    }

    async fn fetch_and_store(
        &self,
        index: usize,
        active: Option<Arc<AtomicBool>>,
    ) -> AppResult<u64> {
        let _permit = self
            .request_slots
            .acquire()
            .await
            .map_err(|_| AppError::msg("audio downloader stopped"))?;

        let document = self.document.lock().await.clone();
        let state = self.app.state::<AppState>();
        let client = state.ensure_client().await?;
        let first_attempt = download::download_chunk(
            &client,
            &state.media_requests,
            &document,
            index,
            active.as_deref(),
        )
        .await;
        let bytes = match first_attempt {
            Ok(bytes) => bytes,
            Err(error) if download::is_file_reference_error(&error) => {
                let _refresh_guard = self.refresh_lock.lock().await;
                let latest = self.document.lock().await.clone();
                let client = state.client().await?;
                match download::download_chunk(
                    &client,
                    &state.media_requests,
                    &latest,
                    index,
                    active.as_deref(),
                )
                .await
                {
                    Ok(bytes) => bytes,
                    Err(retry_error) if download::is_file_reference_error(&retry_error) => {
                        let refreshed = {
                            let state = self.app.state::<AppState>();
                            download::refresh_file_reference(&state, &self.track).await?
                        };
                        *self.document.lock().await = refreshed.clone();
                        let client = state.client().await?;
                        download::download_chunk(
                            &client,
                            &state.media_requests,
                            &refreshed,
                            index,
                            active.as_deref(),
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
