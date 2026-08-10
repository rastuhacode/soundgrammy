use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Weak};

use tauri::AppHandle;
use tokio::sync::Mutex;

use super::TrackStream;
use crate::db::Track;
use crate::error::AppResult;
use crate::telegram::download;

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
