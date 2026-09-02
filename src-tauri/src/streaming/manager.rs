use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use tauri::AppHandle;
use tokio::sync::Mutex;

use super::TrackStream;
use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::telegram::download;

struct PlaybackSession {
    track_id: i64,
    stream: Arc<TrackStream>,
    active: Arc<AtomicBool>,
}

#[derive(Default)]
struct PlaybackSessionState {
    active: HashMap<String, PlaybackSession>,
    /// Handles close-before-open command reordering during very rapid skips.
    closed_before_open: HashSet<String>,
}

#[derive(Default)]
pub struct StreamingManager {
    streams: Mutex<HashMap<i64, Weak<TrackStream>>>,
    playback: Mutex<PlaybackSessionState>,
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
            return Ok(stream);
        }

        let document = download::stored_document(&track)?;
        let stream = TrackStream::create(app, track.clone(), document, destination).await?;
        streams.insert(track.id, Arc::downgrade(&stream));
        Ok(stream)
    }

    pub async fn open_playback(
        &self,
        app: AppHandle,
        track: Track,
        destination: PathBuf,
        session_id: String,
    ) -> AppResult<Arc<TrackStream>> {
        if session_id.is_empty() {
            return Err(AppError::msg("playback stream session id is empty"));
        }
        {
            let mut playback = self.playback.lock().await;
            if playback.closed_before_open.remove(&session_id) {
                return Err(AppError::msg("playback stream session closed"));
            }
        }

        let track_id = track.id;
        let stream = self.start(app, track, destination).await?;
        let active = Arc::new(AtomicBool::new(true));
        {
            let mut playback = self.playback.lock().await;
            if playback.closed_before_open.remove(&session_id) {
                return Err(AppError::msg("playback stream session closed"));
            }
            if let Some(previous) = playback.active.insert(
                session_id.clone(),
                PlaybackSession {
                    track_id,
                    stream: Arc::clone(&stream),
                    active: Arc::clone(&active),
                },
            ) {
                previous.active.store(false, Ordering::Release);
            }
        }

        if let Err(error) = stream
            .ensure_container_mime(Some(Arc::clone(&active)))
            .await
        {
            self.close_playback(&session_id).await;
            return Err(error);
        }
        if !active.load(Ordering::Acquire) {
            return Err(AppError::msg("playback stream session closed"));
        }
        // Reused resumable streams may not download a new chunk during open;
        // replay their ledger so the frontend can make safe backfill choices.
        stream.emit_progress().await;
        Ok(stream)
    }

    pub async fn playback_stream(
        &self,
        session_id: &str,
        track_id: i64,
    ) -> AppResult<(Arc<TrackStream>, Arc<AtomicBool>)> {
        let playback = self.playback.lock().await;
        let session = playback
            .active
            .get(session_id)
            .filter(|session| session.track_id == track_id)
            .ok_or_else(|| AppError::msg("playback stream session not found"))?;
        if !session.active.load(Ordering::Acquire) {
            return Err(AppError::msg("playback stream session closed"));
        }
        Ok((Arc::clone(&session.stream), Arc::clone(&session.active)))
    }

    pub async fn close_playback(&self, session_id: &str) {
        let mut playback = self.playback.lock().await;
        if let Some(session) = playback.active.remove(session_id) {
            session.active.store(false, Ordering::Release);
        } else if !session_id.is_empty() {
            if playback.closed_before_open.len() >= 256 {
                playback.closed_before_open.clear();
            }
            playback.closed_before_open.insert(session_id.to_string());
        }
    }

    pub async fn get(&self, track_id: i64) -> Option<Arc<TrackStream>> {
        let mut streams = self.streams.lock().await;
        streams.retain(|_, stream| stream.strong_count() > 0);
        streams.get(&track_id).and_then(Weak::upgrade)
    }

    /// Destinations / partials for active streams — never delete these on clear/evict.
    pub async fn protected_audio_paths(&self) -> Vec<PathBuf> {
        let mut streams = self.streams.lock().await;
        streams.retain(|_, stream| stream.strong_count() > 0);
        let active: Vec<Arc<TrackStream>> = streams.values().filter_map(Weak::upgrade).collect();
        drop(streams);
        let mut out = Vec::new();
        for stream in active {
            out.extend(stream.protected_paths().await);
        }
        out
    }
}
