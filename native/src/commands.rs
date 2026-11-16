//! Typed service entry points: validate input, call a module, map errors.

use crate::runtime::AppHost;
use serde::Serialize;

use crate::cache;
use crate::db::{LibraryBatch, LibraryPage, Profile, Track};
use crate::display_wake::DisplayWakeState;
use crate::error::AppResult;
use crate::state::AppState;
use crate::telegram::saved_music::{self, SyncMode, SyncResult};

pub mod auth;
pub mod lastfm;
pub mod listen_stats;
pub mod playlists;

// ---- library -------------------------------------------------------------

pub async fn sync_saved_music(
    state: &AppState,
    app: AppHost,
    mode: Option<SyncMode>,
) -> AppResult<SyncResult> {
    saved_music::sync(state, &app, mode.unwrap_or_default()).await
}

pub async fn prepare_full_reindex(state: &AppState) -> AppResult<LibraryBatch> {
    let _guard = state.sync_lock.lock().await;
    let uid = state
        .db
        .load_profile()?
        .ok_or(crate::error::AppError::NotAuthorized)?
        .tg_user_id;
    state.db.request_full_reindex(uid)
}

pub async fn list_tracks(state: &AppState) -> AppResult<Vec<Track>> {
    state
        .db
        .blocking(|db| {
            let profile = db.load_profile()?;
            match profile {
                Some(p) if db.library_status(p.tg_user_id)?.blocking => Ok(Vec::new()),
                Some(p) => db.tracks_by_user(p.tg_user_id),
                None => Ok(Vec::new()),
            }
        })
        .await
}

pub async fn get_profile(state: &AppState) -> AppResult<Option<Profile>> {
    state.db.blocking(|db| db.load_profile()).await
}

pub async fn list_tracks_page(
    state: &AppState,
    offset: i64,
    limit: i64,
    revision: Option<i64>,
) -> AppResult<LibraryPage> {
    state
        .db
        .blocking(move |db| db.tracks_page(db.active_uid()?, offset, limit, revision))
        .await
}

pub async fn sync_status(state: &AppState) -> AppResult<Option<String>> {
    let profile = state.db.load_profile()?;
    match profile {
        Some(p) => state.db.last_sync_at(p.tg_user_id),
        None => Ok(None),
    }
}

/// Holds only user-idle display/system assertions. Explicit or emergency sleep
/// remains controlled by the operating system.
pub async fn set_fullscreen_display_awake(
    state: &DisplayWakeState,
    enabled: bool,
) -> AppResult<()> {
    state.set_enabled(enabled)
}

// ---- media ---------------------------------------------------------------

pub async fn get_track_bounce_profile(
    state: &AppState,
    app: AppHost,
    track_id: i64,
) -> AppResult<crate::bounce_analysis::BounceProfileResponse> {
    Ok(crate::bounce_analysis::profile_for_track(state, &app, track_id).await)
}

pub async fn download_track(state: &AppState, app: AppHost, track_id: i64) -> AppResult<String> {
    let path = cache::ensure_audio(state, &app, track_id).await?;
    Ok(path.to_string_lossy().into_owned())
}

pub async fn prefetch_track(state: &AppState, app: AppHost, track_id: i64) -> AppResult<()> {
    cache::ensure_audio(state, &app, track_id).await?;
    Ok(())
}

/// Explicit "Cache" action — same as prefetch (app cache only).
pub async fn cache_track(state: &AppState, app: AppHost, track_id: i64) -> AppResult<()> {
    let result = cache::cache_tracks(state, &app, &[track_id], None).await?;
    if let Some(failure) = result.failed.first() {
        return Err(crate::error::AppError::msg(&failure.error));
    }
    Ok(())
}

pub async fn cache_tracks(
    state: &AppState,
    app: AppHost,
    track_ids: Vec<i64>,
    job_id: Option<String>,
) -> AppResult<cache::CacheJobResult> {
    cache::cache_tracks(state, &app, &track_ids, job_id.as_deref()).await
}

pub async fn remove_track_from_cache(
    state: &AppState,
    app: AppHost,
    track_id: i64,
) -> AppResult<()> {
    cache::remove_audio(state, &app, track_id).await
}

pub async fn clear_audio_cache(
    state: &AppState,
    app: AppHost,
) -> AppResult<cache::CacheClearResult> {
    cache::clear_audio_cache(state, &app).await
}

pub async fn get_cache_status(state: &AppState) -> AppResult<Vec<i64>> {
    cache::cached_track_ids(state)
}

pub async fn get_cache_settings(state: &AppState) -> AppResult<cache::CacheSettings> {
    cache::get_cache_settings(state)
}

pub async fn set_cache_settings(
    state: &AppState,
    app: AppHost,
    limit_bytes: Option<i64>,
    ttl_secs: Option<i64>,
) -> AppResult<cache::CacheSettings> {
    let settings = cache::set_cache_settings(state, limit_bytes, ttl_secs)?;
    // Apply new TTL / make room under a lower limit.
    let _ = cache::enforce_ttl(state, &app).await;
    let (used, _) = cache::usage_bytes(state).await?;
    let limit = settings.limit_bytes.max(0) as u64;
    if used > limit {
        let _ = cache::evict_for_room(state, &app, 0).await;
    }
    Ok(settings)
}

pub async fn get_cache_usage(state: &AppState) -> AppResult<cache::CacheUsage> {
    cache::get_cache_usage(state).await
}

// ---- proxy ---------------------------------------------------------------

pub async fn get_proxy_settings(
    state: &AppState,
) -> AppResult<crate::proxy_settings::ProxySettingsView> {
    state.proxy_view().await
}

pub async fn set_proxy_settings(
    state: &AppState,
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
    state.proxy_view().await
}

pub async fn parse_proxy_link(link: String) -> AppResult<crate::proxy_settings::ProxySettings> {
    crate::proxy_settings::from_proxy_link(&link)
}

/// Copy a track into the system Downloads folder (user-facing Download).
pub async fn export_track(state: &AppState, app: AppHost, track_id: i64) -> AppResult<String> {
    crate::export::export_track(state, &app, track_id).await
}

pub async fn export_tracks(
    state: &AppState,
    app: AppHost,
    track_ids: Vec<i64>,
) -> AppResult<crate::export::PlaylistDownloadResult> {
    crate::export::export_tracks(state, &app, &track_ids).await
}

/// Download a named playlist folder + M3U8 into Downloads/SoundGrammy (partial success).
pub async fn download_playlist(
    state: &AppState,
    app: AppHost,
    name: String,
    track_ids: Vec<i64>,
    job_id: String,
) -> AppResult<crate::export::PlaylistDownloadResult> {
    crate::export::download_playlist(state, &app, name, &track_ids, job_id).await
}

pub async fn get_track_thumbnail(
    state: &AppState,
    app: AppHost,
    track_id: i64,
    high_quality: Option<bool>,
) -> AppResult<Option<String>> {
    if high_quality.unwrap_or(false) {
        cache::schedule_embedded_artwork(state, &app, track_id).await?;
    }
    let path = cache::ensure_thumbnail(state, track_id, high_quality.unwrap_or(false)).await?;
    Ok(path.map(|p| p.to_string_lossy().into_owned()))
}

pub async fn get_user_avatar(state: &AppState) -> AppResult<Option<String>> {
    let path = cache::ensure_avatar(state).await?;
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

pub async fn track_metadata(state: &AppState, track_id: i64) -> AppResult<TrackMetadata> {
    let uid = require_uid(state)?;
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
    state.db.active_uid()
}

pub mod audio;
