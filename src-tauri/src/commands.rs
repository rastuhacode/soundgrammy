//! Thin Tauri command wrappers: validate input, call a module, map errors.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::cache;
use crate::db::{Profile, Track};
use crate::display_wake::DisplayWakeState;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::streaming;
use crate::telegram::saved_music::{self, SyncResult};

pub(crate) mod auth;
pub(crate) mod listen_stats;
pub(crate) mod playlists;

// ---- library -------------------------------------------------------------

#[tauri::command]
pub async fn sync_saved_music(state: State<'_, AppState>, app: AppHandle) -> AppResult<SyncResult> {
    saved_music::sync(&state, &app).await
}

#[tauri::command]
pub async fn list_tracks(state: State<'_, AppState>) -> AppResult<Vec<Track>> {
    let profile = state.db.load_profile()?;
    match profile {
        Some(p) => state.db.tracks_by_user(p.tg_user_id),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn get_profile(state: State<'_, AppState>) -> AppResult<Option<Profile>> {
    state.db.load_profile()
}

#[tauri::command]
pub async fn sync_status(state: State<'_, AppState>) -> AppResult<Option<String>> {
    let profile = state.db.load_profile()?;
    match profile {
        Some(p) => state.db.last_sync_at(p.tg_user_id),
        None => Ok(None),
    }
}

/// Holds only user-idle display/system assertions. Explicit or emergency sleep
/// remains controlled by the operating system.
#[tauri::command]
pub async fn set_fullscreen_display_awake(
    state: State<'_, DisplayWakeState>,
    enabled: bool,
) -> AppResult<()> {
    state.set_enabled(enabled)
}

// ---- media ---------------------------------------------------------------

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TrackSource {
    Cached {
        path: String,
    },
    Stream {
        #[serde(rename = "trackId")]
        track_id: i64,
        #[serde(rename = "mimeType")]
        mime_type: String,
        total: u64,
    },
}

#[tauri::command]
pub async fn get_track_source(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<TrackSource> {
    let track = cache::require_track(&state, track_id)?;
    let path = cache::audio_path(&state, &track)?;
    if path.exists() {
        return Ok(TrackSource::Cached {
            path: path.to_string_lossy().into_owned(),
        });
    }

    let stream = state.streaming.start(app, track, path).await?;
    Ok(TrackSource::Stream {
        track_id: stream.track_id(),
        mime_type: stream.mime_type().to_string(),
        total: stream.total(),
    })
}

#[tauri::command]
pub async fn get_track_bounce_profile(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<crate::bounce_analysis::BounceProfileResponse> {
    Ok(crate::bounce_analysis::profile_for_track(&state, &app, track_id).await)
}

/// Reads a byte range from an active stream (or a fully cached file).
///
/// `start`/`end` are inclusive. Responses are capped at one streaming chunk
/// so the MSE frontend can append progressively without huge IPC payloads.
#[tauri::command]
pub async fn read_stream_range(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
    start: u64,
    end: u64,
) -> AppResult<tauri::ipc::Response> {
    if end < start {
        return Err(AppError::msg("requested audio range is invalid"));
    }

    let capped_end = start
        .saturating_add(streaming::CHUNK_SIZE.saturating_sub(1))
        .min(end);

    if let Some(stream) = state.streaming.get(track_id).await {
        let last = stream.total().saturating_sub(1);
        if start > last {
            return Err(AppError::msg("requested audio range is invalid"));
        }
        let bytes = stream.read_range(start, capped_end.min(last)).await?;
        return Ok(tauri::ipc::Response::new(bytes));
    }

    let track = cache::require_track(&state, track_id)?;
    let path = cache::audio_path(&state, &track)?;
    if path.exists() {
        let total = tokio::fs::metadata(&path).await?.len();
        if total == 0 {
            return Err(AppError::msg("cached audio file is empty"));
        }
        let last = total.saturating_sub(1);
        if start > last {
            return Err(AppError::msg("requested audio range is invalid"));
        }
        let bytes = streaming::read_file_range(&path, start, capped_end.min(last)).await?;
        return Ok(tauri::ipc::Response::new(bytes));
    }

    // Stream entry expired before cache finalize — restart download and read.
    let stream = state.streaming.start(app, track, path).await?;
    let last = stream.total().saturating_sub(1);
    if start > last {
        return Err(AppError::msg("requested audio range is invalid"));
    }
    let bytes = stream.read_range(start, capped_end.min(last)).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Prioritizes downloading an inclusive byte range for an active stream.
/// Used on seek-ahead so the gap toward the target fills before distant chunks.
#[tauri::command]
pub async fn ensure_stream_range(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
    start: u64,
    end: u64,
) -> AppResult<()> {
    if end < start {
        return Err(AppError::msg("requested audio range is invalid"));
    }

    if let Some(stream) = state.streaming.get(track_id).await {
        let last = stream.total().saturating_sub(1);
        if start > last {
            return Err(AppError::msg("requested audio range is invalid"));
        }
        stream.ensure_range(start, end.min(last)).await?;
        return Ok(());
    }

    let track = cache::require_track(&state, track_id)?;
    let path = cache::audio_path(&state, &track)?;
    if path.exists() {
        return Ok(());
    }

    let stream = state.streaming.start(app, track, path).await?;
    let last = stream.total().saturating_sub(1);
    if start > last {
        return Err(AppError::msg("requested audio range is invalid"));
    }
    stream.ensure_range(start, end.min(last)).await?;
    Ok(())
}

#[tauri::command]
pub async fn download_track(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<String> {
    let path = cache::ensure_audio(&state, &app, track_id).await?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn prefetch_track(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<()> {
    cache::ensure_audio(&state, &app, track_id).await?;
    Ok(())
}

/// Explicit "Cache" action — same as prefetch (app cache only).
#[tauri::command]
pub async fn cache_track(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<()> {
    cache::ensure_audio(&state, &app, track_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn cache_tracks(
    state: State<'_, AppState>,
    app: AppHandle,
    track_ids: Vec<i64>,
    job_id: Option<String>,
) -> AppResult<Vec<i64>> {
    cache::cache_tracks(&state, &app, &track_ids, job_id.as_deref()).await
}

#[tauri::command]
pub async fn remove_track_from_cache(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<()> {
    cache::remove_audio(&state, &app, track_id).await
}

#[tauri::command]
pub async fn clear_audio_cache(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    cache::clear_audio_cache(&state, &app).await
}

#[tauri::command]
pub async fn get_cache_status(state: State<'_, AppState>) -> AppResult<Vec<i64>> {
    cache::cached_track_ids(&state)
}

#[tauri::command]
pub async fn get_cache_settings(state: State<'_, AppState>) -> AppResult<cache::CacheSettings> {
    cache::get_cache_settings(&state)
}

#[tauri::command]
pub async fn set_cache_settings(
    state: State<'_, AppState>,
    app: AppHandle,
    limit_bytes: Option<i64>,
    ttl_secs: Option<i64>,
) -> AppResult<cache::CacheSettings> {
    let settings = cache::set_cache_settings(&state, limit_bytes, ttl_secs)?;
    // Apply new TTL / make room under a lower limit.
    let _ = cache::enforce_ttl(&state, &app).await;
    let (used, _) = cache::usage_bytes(&state).await?;
    let limit = settings.limit_bytes.max(0) as u64;
    if used > limit {
        let _ = cache::evict_for_room(&state, &app, used - limit).await;
    }
    Ok(settings)
}

#[tauri::command]
pub async fn get_cache_usage(state: State<'_, AppState>) -> AppResult<cache::CacheUsage> {
    cache::get_cache_usage(&state).await
}

// ---- proxy ---------------------------------------------------------------

#[tauri::command]
pub async fn get_proxy_settings(
    state: State<'_, AppState>,
) -> AppResult<crate::proxy_settings::ProxySettingsView> {
    let settings = crate::proxy_settings::load(&state.db)?;
    Ok(crate::proxy_settings::ProxySettingsView::from_parts(
        settings,
        state.proxy_active().await,
        state.proxy_last_error().await,
        state.is_telegram_online().await,
    ))
}

#[tauri::command]
pub async fn set_proxy_settings(
    state: State<'_, AppState>,
    enabled: bool,
    server: String,
    port: u16,
    secret: String,
) -> AppResult<crate::proxy_settings::ProxySettingsView> {
    let settings = crate::proxy_settings::ProxySettings {
        enabled,
        server,
        port,
        secret,
    };
    state.apply_proxy_settings(&settings).await?;
    let settings = crate::proxy_settings::load(&state.db)?;
    Ok(crate::proxy_settings::ProxySettingsView::from_parts(
        settings,
        state.proxy_active().await,
        state.proxy_last_error().await,
        state.is_telegram_online().await,
    ))
}

#[tauri::command]
pub async fn parse_proxy_link(link: String) -> AppResult<crate::proxy_settings::ProxySettings> {
    crate::proxy_settings::from_proxy_link(&link)
}

/// Copy a track into the system Downloads folder (user-facing Download).
#[tauri::command]
pub async fn export_track(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<String> {
    crate::export::export_track(&state, &app, track_id).await
}

#[tauri::command]
pub async fn export_tracks(
    state: State<'_, AppState>,
    app: AppHandle,
    track_ids: Vec<i64>,
) -> AppResult<String> {
    crate::export::export_tracks(&state, &app, &track_ids).await
}

/// Download a named playlist folder + M3U8 into Downloads/SoundGrammy (partial success).
#[tauri::command]
pub async fn download_playlist(
    state: State<'_, AppState>,
    app: AppHandle,
    name: String,
    track_ids: Vec<i64>,
    job_id: String,
) -> AppResult<crate::export::PlaylistDownloadResult> {
    crate::export::download_playlist(&state, &app, name, &track_ids, job_id).await
}

#[tauri::command]
pub async fn get_track_thumbnail(
    state: State<'_, AppState>,
    track_id: i64,
    high_quality: Option<bool>,
) -> AppResult<Option<String>> {
    let path = cache::ensure_thumbnail(&state, track_id, high_quality.unwrap_or(false)).await?;
    Ok(path.map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn get_user_avatar(state: State<'_, AppState>) -> AppResult<Option<String>> {
    let path = cache::ensure_avatar(&state).await?;
    Ok(path.map(|p| p.to_string_lossy().into_owned()))
}

#[derive(Serialize)]
pub struct TrackMetadataTrack {
    pub title: Option<String>,
    pub performer: Option<String>,
    pub duration: Option<i64>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(rename = "fileSize")]
    pub file_size: Option<i64>,
    pub source: String,
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(rename = "fileUniqueId")]
    pub file_unique_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Serialize)]
pub struct TrackMetadataDoc {
    pub id: String,
    #[serde(rename = "dcId")]
    pub dc_id: i32,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub size: Option<i64>,
    #[serde(rename = "hasRemoteThumb")]
    pub has_remote_thumb: bool,
    pub attributes: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct TrackMetadata {
    pub track: TrackMetadataTrack,
    pub document: TrackMetadataDoc,
}

#[tauri::command]
pub async fn track_metadata(state: State<'_, AppState>, track_id: i64) -> AppResult<TrackMetadata> {
    let uid = require_uid(&state)?;
    let track = state
        .db
        .track_by_id(track_id, uid)?
        .ok_or_else(|| crate::error::AppError::msg("Track not found"))?;

    let stored: Option<crate::telegram::document::StoredDocument> = track
        .mtproto_document
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok());

    let document = match stored {
        Some(doc) => TrackMetadataDoc {
            id: doc.id.to_string(),
            dc_id: doc.dc_id,
            mime_type: Some(doc.mime_type.clone()),
            size: Some(doc.size_bytes()),
            has_remote_thumb: doc.thumb_size.is_some() || !doc.thumbnails.is_empty(),
            attributes: doc.attributes,
        },
        None => TrackMetadataDoc {
            id: track.file_id.clone(),
            dc_id: 0,
            mime_type: track.mime_type.clone(),
            size: track.file_size,
            has_remote_thumb: false,
            attributes: Vec::new(),
        },
    };

    Ok(TrackMetadata {
        track: TrackMetadataTrack {
            title: track.title,
            performer: track.performer,
            duration: track.duration,
            mime_type: track.mime_type,
            file_size: track.file_size,
            source: track.source,
            file_id: track.file_id,
            file_unique_id: track.file_unique_id,
            created_at: track.created_at,
        },
        document,
    })
}

// ---- playlists -----------------------------------------------------------

fn require_uid(state: &AppState) -> AppResult<i64> {
    state
        .db
        .load_profile()?
        .map(|p| p.tg_user_id)
        .ok_or(crate::error::AppError::NotAuthorized)
}
