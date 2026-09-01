//! Durable sparse-file manifests and stream restoration.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Notify, Semaphore};

use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::telegram::document::StoredDocument;

use super::paths::{partial_metadata_path, partial_metadata_temp_path, partial_path};
use super::progress::{progress_from_state, ChunkSlot, ChunkStatus, StreamState};
use super::{TrackStream, CHUNK_SIZE, FOREGROUND_CONCURRENCY};

const PARTIAL_METADATA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PartialMetadata {
    pub(super) version: u32,
    pub(super) track_id: i64,
    pub(super) file_unique_id: String,
    pub(super) total: u64,
    pub(super) chunk_size: u64,
    pub(super) ready_chunks: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct PartialCacheInfo {
    pub received: u64,
}

pub async fn partial_cache_info(path: &Path) -> Option<PartialCacheInfo> {
    let metadata_path = partial_metadata_path(path);
    let bytes = tokio::fs::read(metadata_path).await.ok()?;
    let metadata: PartialMetadata = serde_json::from_slice(&bytes).ok()?;
    if metadata.version != PARTIAL_METADATA_VERSION
        || metadata.chunk_size != CHUNK_SIZE
        || metadata.total == 0
        || tokio::fs::metadata(path).await.ok()?.len() != metadata.total
    {
        return None;
    }
    let chunk_count = metadata.total.div_ceil(CHUNK_SIZE) as usize;
    let mut ready_chunks = metadata.ready_chunks;
    ready_chunks.sort_unstable();
    if ready_chunks.iter().any(|&index| index >= chunk_count)
        || ready_chunks.windows(2).any(|pair| pair[0] == pair[1])
    {
        return None;
    }
    let received = ready_chunks.iter().fold(0u64, |sum, &index| {
        let offset = index as u64 * CHUNK_SIZE;
        sum.saturating_add((metadata.total - offset).min(CHUNK_SIZE))
    });
    Some(PartialCacheInfo { received })
}

impl TrackStream {
    pub(super) async fn create(
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
        let partial_metadata = partial_metadata_path(&partial);
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let chunk_count = total.div_ceil(CHUNK_SIZE) as usize;
        let restored =
            Self::load_partial_metadata(&track, total, chunk_count, &partial, &partial_metadata)
                .await;
        let ready_chunks = match restored {
            Some(ready) => ready,
            None => {
                let _ = tokio::fs::remove_file(&partial).await;
                let _ = tokio::fs::remove_file(&partial_metadata).await;
                let _ = tokio::fs::remove_file(partial_metadata_temp_path(&partial)).await;
                let file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .read(true)
                    .write(true)
                    .open(&partial)
                    .await?;
                file.set_len(total).await?;
                Vec::new()
            }
        };

        let mut chunks: Vec<ChunkSlot> = (0..chunk_count).map(|_| ChunkSlot::default()).collect();
        let mut received = 0u64;
        for index in ready_chunks {
            chunks[index].status = ChunkStatus::Ready;
            let offset = index as u64 * CHUNK_SIZE;
            received = received.saturating_add((total - offset).min(CHUNK_SIZE));
        }
        let mime_type = document.mime_type.clone();
        let stream = Arc::new(Self {
            track,
            document: Mutex::new(document),
            mime_type: RwLock::new(mime_type),
            destination,
            final_path: Mutex::new(None),
            partial,
            partial_metadata,
            total,
            app,
            state: Mutex::new(StreamState {
                chunks,
                received,
                terminal_error: None,
            }),
            request_slots: Semaphore::new(FOREGROUND_CONCURRENCY),
            write_lock: Mutex::new(()),
            refresh_lock: Mutex::new(()),
            finalize_lock: Mutex::new(()),
            metadata_lock: Mutex::new(()),
            completion: Notify::new(),
        });
        stream.persist_partial_metadata().await?;
        stream.emit_progress().await;
        // A previous process may have downloaded the last chunk immediately
        // before exit but not completed the rename.
        stream.try_finalize().await?;
        Ok(stream)
    }

    async fn load_partial_metadata(
        track: &Track,
        total: u64,
        chunk_count: usize,
        partial: &Path,
        metadata_path: &Path,
    ) -> Option<Vec<usize>> {
        if !partial.exists() || !metadata_path.exists() {
            return None;
        }
        let file_len = tokio::fs::metadata(partial).await.ok()?.len();
        if file_len != total {
            return None;
        }
        let bytes = tokio::fs::read(metadata_path).await.ok()?;
        let metadata: PartialMetadata = serde_json::from_slice(&bytes).ok()?;
        if metadata.version != PARTIAL_METADATA_VERSION
            || metadata.track_id != track.id
            || metadata.file_unique_id != track.file_unique_id
            || metadata.total != total
            || metadata.chunk_size != CHUNK_SIZE
            || metadata
                .ready_chunks
                .iter()
                .any(|&index| index >= chunk_count)
        {
            return None;
        }
        let mut ready = metadata.ready_chunks;
        ready.sort_unstable();
        ready.dedup();
        Some(ready)
    }

    pub(super) async fn persist_partial_metadata(&self) -> AppResult<()> {
        if self.destination.exists() || self.final_path.lock().await.is_some() {
            return Ok(());
        }
        let _guard = self.metadata_lock.lock().await;
        if self.destination.exists() || self.final_path.lock().await.is_some() {
            return Ok(());
        }
        let ready_chunks = self.state.lock().await.ready_indices();
        let metadata = PartialMetadata {
            version: PARTIAL_METADATA_VERSION,
            track_id: self.track.id,
            file_unique_id: self.track.file_unique_id.clone(),
            total: self.total,
            chunk_size: CHUNK_SIZE,
            ready_chunks,
        };
        let bytes = serde_json::to_vec(&metadata)?;
        let temp = partial_metadata_temp_path(&self.partial);
        tokio::fs::write(&temp, bytes).await?;
        if let Err(error) = tokio::fs::rename(&temp, &self.partial_metadata).await {
            // Windows does not consistently replace an existing file with
            // rename. Losing the old manifest in this fallback is safe: an
            // interrupted write merely discards the partial on the next open.
            if matches!(
                error.kind(),
                std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::PermissionDenied
            ) {
                let _ = tokio::fs::remove_file(&self.partial_metadata).await;
                tokio::fs::rename(&temp, &self.partial_metadata).await?;
            } else {
                return Err(error.into());
            }
        }
        Ok(())
    }

    pub async fn emit_progress(&self) {
        let progress = {
            let state = self.state.lock().await;
            progress_from_state(self.track.id, self.total, &state)
        };
        let _ = self.app.emit("download:progress", progress);
    }
}
