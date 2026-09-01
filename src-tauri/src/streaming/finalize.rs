//! Atomic completion, MIME correction, and race-safe range readback.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{Emitter, Manager};
use tokio::io::AsyncReadExt;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::{extension_for_mime, sniff_container_mime};

use super::paths::{complete_path, partial_metadata_temp_path, resolve_stream_read_path};
use super::progress::{progress_from_state, ChunkStatus};
use super::{read_file_range, TrackStream};

impl TrackStream {
    /// Download the file header and correct MIME when Telegram's label is wrong.
    /// Must run before the frontend opens an MSE SourceBuffer.
    pub async fn ensure_container_mime(
        self: &Arc<Self>,
        active: Option<Arc<AtomicBool>>,
    ) -> AppResult<()> {
        let last = 15u64.min(self.total.saturating_sub(1));
        let header = self.read_range(0, last, active).await?;
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
        self.document.lock().await.mime_type = sniffed.to_string();
        Ok(())
    }

    async fn persist_current_document_mime(&self) {
        let document = self.document.lock().await.clone();
        let Ok(json) = serde_json::to_string(&document) else {
            return;
        };
        let state = self.app.state::<AppState>();
        if let Ok(Some(profile)) = state.db.load_profile() {
            let _ =
                state
                    .db
                    .update_track_mime(self.track.id, profile.tg_user_id, &document.mime_type);
            let _ = state
                .db
                .update_track_document(self.track.id, profile.tg_user_id, &json);
        }
    }

    pub(super) async fn try_finalize(&self) -> AppResult<()> {
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
        let _ = tokio::fs::remove_file(&self.partial_metadata).await;
        let _ = tokio::fs::remove_file(partial_metadata_temp_path(&self.partial)).await;
        let state = self.app.state::<AppState>();
        let _ = state.db.mark_audio_cache_automatic(self.track.id);

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
        self.persist_current_document_mime().await;

        Ok(path)
    }

    pub async fn read_range(
        self: &Arc<Self>,
        start: u64,
        end: u64,
        active: Option<Arc<AtomicBool>>,
    ) -> AppResult<Vec<u8>> {
        self.ensure_range(start, end, active).await?;
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
