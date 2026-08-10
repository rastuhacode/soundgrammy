//! Progressive, range-prioritized audio streaming backed by Telegram file parts.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use futures_util::stream::{FuturesUnordered, StreamExt};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, Notify, Semaphore};

use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::{extension_for_mime, sniff_container_mime, StoredDocument};
use crate::telegram::download;

pub const CHUNK_SIZE: u64 = 128 * 1024;
const FOREGROUND_CONCURRENCY: usize = 4;

mod manager;
mod paths;
mod progress;
mod protocol;
#[cfg(test)]
mod tests;

pub use manager::StreamingManager;
pub use protocol::{protocol_response, read_file_range};

use paths::{complete_path, partial_path, resolve_stream_read_path};
use progress::{progress_from_state, ChunkSlot, ChunkStatus, StreamState};

pub struct TrackStream {
    track: Track,
    document: Mutex<StoredDocument>,
    /// Container MIME after header sniff (Telegram metadata is sometimes wrong).
    mime_type: RwLock<String>,
    destination: PathBuf,
    /// Set after finalize when the on-disk extension was corrected from a sniff.
    final_path: Mutex<Option<PathBuf>>,
    partial: PathBuf,
    total: u64,
    app: AppHandle,
    state: Mutex<StreamState>,
    request_slots: Semaphore,
    write_lock: Mutex<()>,
    refresh_lock: Mutex<()>,
    finalize_lock: Mutex<()>,
    completion: Notify,
}

impl TrackStream {
    async fn create(
        app: AppHandle,
        track: Track,
        document: StoredDocument,
        destination: PathBuf,
    ) -> AppResult<Arc<Self>> {
        let total = u64::try_from(document.size_bytes())
            .map_err(|_| AppError::msg("track has an invalid file size"))?;
        if total == 0 {
            return Err(AppError::msg("track has an empty file"));
        }

        let partial = partial_path(&destination);
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if partial.exists() {
            tokio::fs::remove_file(&partial).await?;
        }

        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&partial)
            .await?;
        file.set_len(total).await?;

        let chunk_count = total.div_ceil(CHUNK_SIZE) as usize;
        let mime_type = document.mime_type.clone();
        Ok(Arc::new(Self {
            track,
            document: Mutex::new(document),
            mime_type: RwLock::new(mime_type),
            destination,
            final_path: Mutex::new(None),
            partial,
            total,
            app,
            state: Mutex::new(StreamState {
                chunks: (0..chunk_count).map(|_| ChunkSlot::default()).collect(),
                received: 0,
                terminal_error: None,
            }),
            request_slots: Semaphore::new(FOREGROUND_CONCURRENCY),
            write_lock: Mutex::new(()),
            refresh_lock: Mutex::new(()),
            finalize_lock: Mutex::new(()),
            completion: Notify::new(),
        }))
    }

    pub fn track_id(&self) -> i64 {
        self.track.id
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn mime_type(&self) -> String {
        self.mime_type
            .read()
            .map(|value| value.clone())
            .unwrap_or_else(|_| {
                self.track
                    .mime_type
                    .clone()
                    .unwrap_or_else(|| "audio/mpeg".into())
            })
    }

    /// Paths that must not be deleted while this stream is active.
    pub async fn protected_paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![self.destination.clone(), self.partial.clone()];
        if let Some(final_path) = self.final_path.lock().await.clone() {
            paths.push(final_path);
        }
        paths
    }

    /// Download the file header and correct MIME when Telegram's label is wrong.
    /// Must run before the frontend opens an MSE SourceBuffer.
    pub async fn ensure_container_mime(self: &Arc<Self>) -> AppResult<()> {
        let last = 15u64.min(self.total.saturating_sub(1));
        self.ensure_range(0, last).await?;
        let header = read_file_range(&self.partial, 0, last).await?;
        let Some(sniffed) = sniff_container_mime(&header) else {
            return Ok(());
        };
        self.apply_sniffed_mime(sniffed).await
    }

    async fn apply_sniffed_mime(&self, sniffed: &str) -> AppResult<()> {
        let current = self.mime_type();
        if extension_for_mime(&current) == extension_for_mime(sniffed) {
            return Ok(());
        }

        if let Ok(mut guard) = self.mime_type.write() {
            *guard = sniffed.to_string();
        }
        {
            let mut doc = self.document.lock().await;
            doc.mime_type = sniffed.to_string();
            if let Ok(json) = serde_json::to_string(&*doc) {
                let state = self.app.state::<AppState>();
                if let Some(uid) = state.db.load_profile()?.map(|p| p.tg_user_id) {
                    let _ = state.db.update_track_mime(self.track.id, uid, sniffed);
                    let _ = state.db.update_track_document(self.track.id, uid, &json);
                }
            }
        }
        Ok(())
    }

    fn start_background(self: &Arc<Self>) {
        let stream = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let chunk_count = stream.state.lock().await.chunks.len();
            for index in 0..chunk_count {
                if let Err(error) = stream.ensure_chunk(index).await {
                    let mut state = stream.state.lock().await;
                    state.terminal_error = Some(error.to_string());
                    stream.completion.notify_waiters();
                    return;
                }
            }
        });
    }

    async fn restart_background_if_failed(self: &Arc<Self>) {
        let should_restart = self.state.lock().await.terminal_error.take().is_some();
        if should_restart {
            self.start_background();
        }
    }

    pub async fn ensure_range(self: &Arc<Self>, start: u64, end: u64) -> AppResult<()> {
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
            requests.push(async move { stream.ensure_chunk(index).await });
        }
        while let Some(result) = requests.next().await {
            result?;
        }
        Ok(())
    }

    async fn ensure_chunk(self: &Arc<Self>, index: usize) -> AppResult<()> {
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
            return self.wait_for_chunk(index, &notify).await;
        }

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
        let _ = self.app.emit("download:progress", progress);

        match result {
            Ok(_) => self.try_finalize().await,
            Err(error) => Err(error),
        }
    }

    async fn wait_for_chunk(&self, index: usize, notify: &Notify) -> AppResult<()> {
        let mut notified = Box::pin(notify.notified());
        loop {
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
        Ok(bytes.len() as u64)
    }

    async fn try_finalize(&self) -> AppResult<()> {
        if self.final_path.lock().await.is_some() {
            self.completion.notify_waiters();
            return Ok(());
        }

        let ready = {
            let state = self.state.lock().await;
            state.received == self.total
                && state
                    .chunks
                    .iter()
                    .all(|slot| slot.status == ChunkStatus::Ready)
        };
        if !ready && !self.destination.exists() {
            return Ok(());
        }

        let _guard = self.finalize_lock.lock().await;
        if self.final_path.lock().await.is_some() {
            self.completion.notify_waiters();
            return Ok(());
        }

        if !self.destination.exists() {
            match tokio::fs::rename(&self.partial, &self.destination).await {
                Ok(()) => {}
                Err(_) => {
                    let complete = complete_path(&self.destination);
                    tokio::fs::copy(&self.partial, &complete).await?;
                    tokio::fs::rename(&complete, &self.destination).await?;
                    let _ = tokio::fs::remove_file(&self.partial).await;
                }
            }
        }

        let path = self.correct_cached_extension().await?;
        *self.final_path.lock().await = Some(path);

        let state = self.state.lock().await;
        let _ = self.app.emit(
            "download:progress",
            progress_from_state(self.track.id, self.total, &state),
        );
        drop(state);
        let _ = self.app.emit(
            "cache:changed",
            crate::cache::CacheChanged {
                track_ids: vec![self.track.id],
                cached: true,
                cleared: false,
            },
        );
        self.completion.notify_waiters();
        Ok(())
    }

    /// Telegram MIME is sometimes wrong (e.g. Opus-in-WebM as `audio/mpeg`).
    /// Fix the cache extension once when the download completes.
    async fn correct_cached_extension(&self) -> AppResult<PathBuf> {
        let path = self.destination.clone();
        let mut header = vec![0u8; 16];
        {
            let mut file = tokio::fs::File::open(&path).await?;
            let read = file.read(&mut header).await?;
            header.truncate(read);
        }
        let Some(sniffed) = sniff_container_mime(&header) else {
            return Ok(path);
        };

        let correct_ext = extension_for_mime(sniffed);
        let current_ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let path = if current_ext != correct_ext {
            let renamed = path.with_extension(correct_ext);
            if renamed != path {
                if renamed.exists() {
                    let _ = tokio::fs::remove_file(&path).await;
                } else {
                    tokio::fs::rename(&path, &renamed).await?;
                }
            }
            renamed
        } else {
            path
        };

        let declared = self.mime_type();
        if extension_for_mime(&declared) != correct_ext {
            self.apply_sniffed_mime(sniffed).await?;
        }

        Ok(path)
    }

    pub async fn read_range(self: &Arc<Self>, start: u64, end: u64) -> AppResult<Vec<u8>> {
        self.ensure_range(start, end).await?;
        // Snapshot the path under the finalize lock so we do not start a read
        // against a path that try_finalize is about to rename. If finalize
        // still races the IO (rename between snapshot and open), retry once
        // with a fresh path — MSE keeps appending after download:complete and
        // a failed read would pause playback via onError.
        let path = {
            let _guard = self.finalize_lock.lock().await;
            self.resolve_read_path().await
        };
        match read_file_range(&path, start, end).await {
            Ok(bytes) => Ok(bytes),
            Err(err) => {
                let retry_path = {
                    let _guard = self.finalize_lock.lock().await;
                    self.resolve_read_path().await
                };
                if retry_path != path {
                    read_file_range(&retry_path, start, end).await
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn resolve_read_path(&self) -> PathBuf {
        resolve_stream_read_path(
            self.final_path.lock().await.clone(),
            &self.destination,
            &self.partial,
        )
    }

    pub async fn wait_complete(&self) -> AppResult<PathBuf> {
        let mut notified = Box::pin(self.completion.notified());
        loop {
            notified.as_mut().enable();
            if let Some(path) = self.final_path.lock().await.clone() {
                return Ok(path);
            }
            if self.destination.exists() {
                return Ok(self.destination.clone());
            }
            if let Some(error) = self.state.lock().await.terminal_error.clone() {
                return Err(AppError::msg(error));
            }
            notified.as_mut().await;
            notified.set(self.completion.notified());
        }
    }
}
