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

impl TrackStream {
    pub async fn download_complete(self: &Arc<Self>) -> AppResult<PathBuf> {
        self.download_complete_with_active(None).await
    }

    pub async fn download_complete_for_playback(
        self: &Arc<Self>,
        active: Arc<AtomicBool>,
    ) -> AppResult<PathBuf> {
        self.download_complete_with_active(Some(active)).await
    }

    /// Fill only the file prefix skipped by MPEG playback (typically ID3v2).
    /// Requests are deliberately sequential and remain owned by the playback
    /// session, so foreground audio never competes with a metadata burst.
    pub async fn backfill_id3_for_playback(
        self: &Arc<Self>,
        active: Arc<AtomicBool>,
    ) -> AppResult<()> {
        let header_end = 9.min(self.total.saturating_sub(1));
        if header_end < 9 {
            return Ok(());
        }
        let header = self
            .read_range(0, header_end, Some(Arc::clone(&active)))
            .await?;
        let Some(prefix_end) = id3v2_tag_byte_length(&header, self.total) else {
            return Ok(());
        };
        let chunk_count = Self::prefix_chunk_count(prefix_end, self.total)?;
        for index in 0..chunk_count {
            Self::require_active(Some(&active))?;
            self.ensure_chunk(index, Some(Arc::clone(&active))).await?;
        }
        Self::require_active(Some(&active))?;
        Ok(())
    }

    pub(super) fn prefix_chunk_count(prefix_end: u64, total: u64) -> AppResult<usize> {
        if prefix_end > total {
            return Err(AppError::msg("metadata prefix range is invalid"));
        }
        Ok(prefix_end.div_ceil(CHUNK_SIZE) as usize)
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
            return self.wait_for_chunk(index, &notify, active.as_deref()).await;
        }

        Self::require_active(active.as_deref())?;
        let result = self.fetch_and_store(index).await;
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

        match result {
            Ok(_) => {
                self.try_finalize().await?;
                if self.final_path.lock().await.is_none() {
                    metadata_result?;
                }
                Ok(())
            }
            Err(error) => Err(error),
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
    ) -> AppResult<()> {
        let mut notified = Box::pin(notify.notified());
        loop {
            Self::require_active(active)?;
            notified.as_mut().enable();
            {
                let state = self.state.lock().await;
                let slot = &state.chunks[index];
                match slot.status {
                    ChunkStatus::Ready => return Ok(()),
                    ChunkStatus::Missing => {
                        return Err(AppError::msg(
                            slot.error
                                .clone()
                                .unwrap_or_else(|| "audio chunk download failed".into()),
                        ));
                    }
                    ChunkStatus::Loading => {}
                }
            }
            notified.as_mut().await;
            notified.set(notify.notified());
        }
    }

    async fn fetch_and_store(&self, index: usize) -> AppResult<u64> {
        let _permit = self
            .request_slots
            .acquire()
            .await
            .map_err(|_| AppError::msg("audio downloader stopped"))?;

        let document = self.document.lock().await.clone();
        let client = self.app.state::<AppState>().client().await?;
        let first_attempt = download::download_chunk(&client, &document, index).await;
        let bytes = match first_attempt {
            Ok(bytes) => bytes,
            Err(error) if download::is_file_reference_error(&error) => {
                let _refresh_guard = self.refresh_lock.lock().await;
                let latest = self.document.lock().await.clone();
                let client = self.app.state::<AppState>().client().await?;
                match download::download_chunk(&client, &latest, index).await {
                    Ok(bytes) => bytes,
                    Err(retry_error) if download::is_file_reference_error(&retry_error) => {
                        let refreshed = {
                            let state = self.app.state::<AppState>();
                            download::refresh_file_reference(&state, &self.track).await?
                        };
                        *self.document.lock().await = refreshed.clone();
                        let client = self.app.state::<AppState>().client().await?;
                        download::download_chunk(&client, &refreshed, index).await?
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

pub(super) fn id3v2_tag_byte_length(header: &[u8], total: u64) -> Option<u64> {
    if header.len() < 10 || &header[..3] != b"ID3" {
        return None;
    }
    let flags = header[5];
    let size = ((u64::from(header[6] & 0x7f)) << 21)
        | ((u64::from(header[7] & 0x7f)) << 14)
        | ((u64::from(header[8] & 0x7f)) << 7)
        | u64::from(header[9] & 0x7f);
    let footer = if flags & 0x10 != 0 { 10 } else { 0 };
    let tag_end = 10u64.checked_add(size)?.checked_add(footer)?;
    (tag_end < total).then_some(tag_end)
}
