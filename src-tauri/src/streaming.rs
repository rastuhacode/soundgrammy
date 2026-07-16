//! Progressive, range-prioritized audio streaming backed by Telegram file parts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, Weak};

use futures_util::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, Notify, Semaphore};

use crate::cache;
use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::{extension_for_mime, sniff_container_mime, StoredDocument};
use crate::telegram::download;

pub const CHUNK_SIZE: u64 = 128 * 1024;
const FOREGROUND_CONCURRENCY: usize = 4;
const MAX_PROTOCOL_RESPONSE: u64 = CHUNK_SIZE;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LoadedRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    #[serde(rename = "trackId")]
    track_id: i64,
    received: u64,
    total: u64,
    ranges: Vec<LoadedRange>,
    complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkStatus {
    Missing,
    Loading,
    Ready,
}

struct ChunkSlot {
    status: ChunkStatus,
    error: Option<String>,
    notify: Arc<Notify>,
}

impl Default for ChunkSlot {
    fn default() -> Self {
        Self {
            status: ChunkStatus::Missing,
            error: None,
            notify: Arc::new(Notify::new()),
        }
    }
}

struct StreamState {
    chunks: Vec<ChunkSlot>,
    received: u64,
    terminal_error: Option<String>,
}

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
        let first_attempt =
            download::download_chunk(&self.app.state::<AppState>().client, &document, index).await;
        let bytes = match first_attempt {
            Ok(bytes) => bytes,
            Err(error) if download::is_file_reference_error(&error) => {
                let _refresh_guard = self.refresh_lock.lock().await;
                let latest = self.document.lock().await.clone();
                match download::download_chunk(&self.app.state::<AppState>().client, &latest, index)
                    .await
                {
                    Ok(bytes) => bytes,
                    Err(retry_error) if download::is_file_reference_error(&retry_error) => {
                        let refreshed = {
                            let state = self.app.state::<AppState>();
                            download::refresh_file_reference(&state, &self.track).await?
                        };
                        *self.document.lock().await = refreshed.clone();
                        download::download_chunk(
                            &self.app.state::<AppState>().client,
                            &refreshed,
                            index,
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
        let path = if let Some(path) = self.final_path.lock().await.clone() {
            path
        } else if self.destination.exists() {
            self.destination.clone()
        } else {
            self.partial.clone()
        };
        read_file_range(&path, start, end).await
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

#[derive(Default)]
pub struct StreamingManager {
    streams: Mutex<HashMap<i64, Weak<TrackStream>>>,
}

impl StreamingManager {
    pub async fn start(
        &self,
        app: AppHandle,
        track: Track,
        destination: PathBuf,
    ) -> AppResult<Arc<TrackStream>> {
        let mut streams = self.streams.lock().await;
        streams.retain(|_, stream| stream.strong_count() > 0);
        if let Some(stream) = streams.get(&track.id).and_then(Weak::upgrade) {
            drop(streams);
            stream.restart_background_if_failed().await;
            stream.ensure_container_mime().await?;
            return Ok(stream);
        }

        let document = download::stored_document(&track)?;
        let stream = TrackStream::create(app, track.clone(), document, destination).await?;
        streams.insert(track.id, Arc::downgrade(&stream));
        drop(streams);
        stream.start_background();
        stream.ensure_container_mime().await?;
        Ok(stream)
    }

    pub async fn get(&self, track_id: i64) -> Option<Arc<TrackStream>> {
        let mut streams = self.streams.lock().await;
        streams.retain(|_, stream| stream.strong_count() > 0);
        streams.get(&track_id).and_then(Weak::upgrade)
    }
}

pub async fn protocol_response(app: &AppHandle, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    match try_protocol_response(app, request).await {
        Ok(response) => response,
        Err(error) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(error.to_string().into_bytes())
            .expect("valid streaming error response"),
    }
}

async fn try_protocol_response(
    app: &AppHandle,
    request: Request<Vec<u8>>,
) -> AppResult<Response<Vec<u8>>> {
    let track_id = request
        .uri()
        .path()
        .trim_matches('/')
        .rsplit('/')
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| AppError::msg("invalid streaming track id"))?;
    let state = app.state::<AppState>();
    let (stream, cached_path, total, mime_type) =
        if let Some(stream) = state.streaming.get(track_id).await {
            let total = stream.total();
            let mime_type = stream.mime_type().to_string();
            (Some(stream), None, total, mime_type)
        } else {
            let Ok(track) = cache::require_track(&state, track_id) else {
                return Ok(not_found_response());
            };
            let path = cache::audio_path(&state, &track)?;
            if !path.exists() {
                return Ok(not_found_response());
            }
            let total = tokio::fs::metadata(&path).await?.len();
            if total == 0 {
                return Err(AppError::msg("cached audio file is empty"));
            }
            let mime_type = track.mime_type.unwrap_or_else(|| "audio/mpeg".into());
            (None, Some(path), total, mime_type)
        };

    if request.method() == Method::HEAD {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::CONTENT_LENGTH, total)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, "no-store")
            .body(Vec::new())
            .expect("valid streaming HEAD response"));
    };

    let (start, requested_end) =
        match parse_single_range(request.headers().get(header::RANGE), total) {
            Ok(range) => range,
            Err(()) => {
                return Ok(Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                    .header(header::ACCEPT_RANGES, "bytes")
                    .body(Vec::new())
                    .expect("valid range error response"));
            }
        };
    let end = requested_end.min(start + MAX_PROTOCOL_RESPONSE - 1);
    let bytes = if let Some(stream) = stream {
        stream.read_range(start, end).await?
    } else {
        read_file_range(
            cached_path
                .as_deref()
                .expect("cached path exists without an active stream"),
            start,
            end,
        )
        .await?
    };

    Ok(Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, mime_type)
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        )
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(bytes)
        .expect("valid streaming range response"))
}

fn not_found_response() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .expect("valid not-found response")
}

pub async fn read_file_range(path: &Path, start: u64, end: u64) -> AppResult<Vec<u8>> {
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut bytes = vec![0; (end - start + 1) as usize];
    file.read_exact(&mut bytes).await?;
    Ok(bytes)
}

fn parse_single_range(
    header_value: Option<&tauri::http::HeaderValue>,
    total: u64,
) -> Result<(u64, u64), ()> {
    let Some(value) = header_value else {
        return Ok((0, total.saturating_sub(1)));
    };
    let value = value.to_str().map_err(|_| ())?;
    let range = value.strip_prefix("bytes=").ok_or(())?;
    if range.contains(',') {
        return Err(());
    }
    let (start, end) = range.split_once('-').ok_or(())?;

    if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        let start = total.saturating_sub(suffix);
        return Ok((start, total.saturating_sub(1)));
    }

    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= total {
        return Err(());
    }
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(total - 1)
    };
    if end < start {
        return Err(());
    }
    Ok((start, end))
}

fn progress_from_state(track_id: i64, total: u64, state: &StreamState) -> DownloadProgress {
    DownloadProgress {
        track_id,
        received: state.received,
        total,
        ranges: loaded_ranges(&state.chunks, total),
        complete: state.received == total,
    }
}

fn loaded_ranges(chunks: &[ChunkSlot], total: u64) -> Vec<LoadedRange> {
    let mut ranges = Vec::new();
    let mut start = None;

    for (index, slot) in chunks.iter().enumerate() {
        let ready = slot.status == ChunkStatus::Ready;
        match (start, ready) {
            (None, true) => start = Some(index as u64 * CHUNK_SIZE),
            (Some(range_start), false) => {
                ranges.push(LoadedRange {
                    start: range_start,
                    end: index as u64 * CHUNK_SIZE,
                });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(range_start) = start {
        ranges.push(LoadedRange {
            start: range_start,
            end: total,
        });
    }
    ranges
}

fn partial_path(destination: &Path) -> PathBuf {
    let mut value = destination.as_os_str().to_owned();
    value.push(".part");
    PathBuf::from(value)
}

fn complete_path(destination: &Path) -> PathBuf {
    let mut value = destination.as_os_str().to_owned();
    value.push(".complete");
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_adjacent_ready_chunks_into_byte_ranges() {
        let mut chunks: Vec<ChunkSlot> = (0..5).map(|_| ChunkSlot::default()).collect();
        chunks[0].status = ChunkStatus::Ready;
        chunks[1].status = ChunkStatus::Ready;
        chunks[3].status = ChunkStatus::Ready;

        assert_eq!(
            loaded_ranges(&chunks, 5 * CHUNK_SIZE),
            vec![
                LoadedRange {
                    start: 0,
                    end: 2 * CHUNK_SIZE,
                },
                LoadedRange {
                    start: 3 * CHUNK_SIZE,
                    end: 4 * CHUNK_SIZE,
                },
            ]
        );
    }

    #[test]
    fn clamps_the_last_loaded_range_to_file_size() {
        let mut chunks: Vec<ChunkSlot> = (0..2).map(|_| ChunkSlot::default()).collect();
        chunks[1].status = ChunkStatus::Ready;
        let total = CHUNK_SIZE + 17;

        assert_eq!(
            loaded_ranges(&chunks, total),
            vec![LoadedRange {
                start: CHUNK_SIZE,
                end: total,
            }]
        );
    }

    #[test]
    fn parses_open_and_suffix_http_ranges() {
        let open = tauri::http::HeaderValue::from_static("bytes=131072-");
        let suffix = tauri::http::HeaderValue::from_static("bytes=-256");

        assert_eq!(
            parse_single_range(Some(&open), 1_000_000),
            Ok((131_072, 999_999))
        );
        assert_eq!(parse_single_range(Some(&suffix), 1_000), Ok((744, 999)));
    }

    #[test]
    fn rejects_multiple_or_out_of_bounds_ranges() {
        let multiple = tauri::http::HeaderValue::from_static("bytes=0-1,4-5");
        let outside = tauri::http::HeaderValue::from_static("bytes=1000-");

        assert_eq!(parse_single_range(Some(&multiple), 1_000), Err(()));
        assert_eq!(parse_single_range(Some(&outside), 1_000), Err(()));
    }
}
