//! Progressive, range-prioritized audio streaming backed by Telegram file parts.
//!
//! This module is the shared façade/state definition. Implementation details
//! live in focused siblings: partial restoration, transfer, and finalization.

use std::path::PathBuf;
use std::sync::RwLock;

use tauri::AppHandle;
use tokio::sync::{Mutex, Notify, Semaphore};

use crate::db::Track;
use crate::telegram::document::StoredDocument;

pub const CHUNK_SIZE: u64 = 128 * 1024;
pub(super) const FOREGROUND_CONCURRENCY: usize = 4;

mod file;
mod finalize;
mod manager;
mod partial_cache;
mod paths;
mod progress;
#[cfg(test)]
mod tests;
mod transfer;

pub use file::read_file_range;
pub use manager::StreamingManager;
pub use partial_cache::partial_cache_info;

use paths::partial_metadata_temp_path;
use progress::StreamState;

pub struct TrackStream {
    track: Track,
    document: Mutex<StoredDocument>,
    /// Container MIME after header sniff (Telegram metadata is sometimes wrong).
    mime_type: RwLock<String>,
    destination: PathBuf,
    /// Set after finalize when the on-disk extension was corrected from a sniff.
    final_path: Mutex<Option<PathBuf>>,
    partial: PathBuf,
    partial_metadata: PathBuf,
    total: u64,
    app: AppHandle,
    state: Mutex<StreamState>,
    request_slots: Semaphore,
    write_lock: Mutex<()>,
    refresh_lock: Mutex<()>,
    finalize_lock: Mutex<()>,
    metadata_lock: Mutex<()>,
    completion: Notify,
}

impl TrackStream {
    pub fn total(&self) -> u64 {
        self.total
    }

    pub async fn received(&self) -> u64 {
        self.state.lock().await.received
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
        let mut paths = vec![
            self.destination.clone(),
            self.partial.clone(),
            self.partial_metadata.clone(),
            partial_metadata_temp_path(&self.partial),
        ];
        if let Some(final_path) = self.final_path.lock().await.clone() {
            paths.push(final_path);
        }
        paths
    }
}

impl TrackStream {
    /// Storage-only observer: waits for verified chunks without scheduling downloads.
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    pub async fn read_downloaded_range(
        self: &std::sync::Arc<Self>,
        start: u64,
        end: u64,
        active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> crate::error::AppResult<Vec<u8>> {
        if start > end || end >= self.total {
            return Err(crate::error::AppError::msg("invalid observer range"));
        }
        loop {
            Self::require_active(Some(&active))?;
            let ready = {
                let state = self.state.lock().await;
                state.chunks[(start / CHUNK_SIZE) as usize..=(end / CHUNK_SIZE) as usize]
                    .iter()
                    .all(|slot| slot.status == progress::ChunkStatus::Ready)
            };
            if ready {
                return self.read_range(start, end, Some(active)).await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
}
