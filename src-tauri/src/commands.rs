//! Thin Tauri command wrappers: validate input, call a module, map errors.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::cache;
use crate::db::{CustomPlaylistSummary, PlaylistsBundle, Profile, Track};
use crate::error::AppResult;
use crate::state::AppState;
use crate::telegram::auth::{self, AuthOutcome, AuthUser, QrOutcome};
use crate::telegram::saved_music::{self, SyncResult};

#[derive(Serialize)]
pub struct AuthStatus {
    pub authorized: bool,
    pub user: Option<AuthUser>,
}

// ---- auth ----------------------------------------------------------------

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> AppResult<AuthStatus> {
    let authorized = crate::telegram::client::is_authorized(&state.client).await?;
    if !authorized {
        return Ok(AuthStatus {
            authorized: false,
            user: None,
        });
    }
    let user = auth::fetch_self(&state.client).await?;
    state.db.save_profile(
        user.id,
        &user.first_name,
        user.last_name.as_deref(),
        user.username.as_deref(),
        user.phone.as_deref(),
    )?;
    Ok(AuthStatus {
        authorized: true,
        user: Some(user),
    })
}

#[tauri::command]
pub async fn phone_send_code(state: State<'_, AppState>, phone: String) -> AppResult<()> {
    auth::phone_send_code(&state, phone.trim()).await
}

#[tauri::command]
pub async fn phone_sign_in(state: State<'_, AppState>, code: String) -> AppResult<AuthOutcome> {
    auth::phone_sign_in(&state, code.trim()).await
}

#[tauri::command]
pub async fn phone_check_password(
    state: State<'_, AppState>,
    password: String,
) -> AppResult<AuthUser> {
    auth::check_password(&state, &password).await
}

#[tauri::command]
pub async fn qr_start(state: State<'_, AppState>) -> AppResult<QrOutcome> {
    auth::qr_export(&state).await
}

#[tauri::command]
pub async fn qr_poll(state: State<'_, AppState>) -> AppResult<QrOutcome> {
    auth::qr_export(&state).await
}

#[tauri::command]
pub async fn qr_check_password(
    state: State<'_, AppState>,
    password: String,
) -> AppResult<AuthUser> {
    auth::qr_check_password(&state, &password).await
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> AppResult<()> {
    auth::logout(&state).await
}

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

#[tauri::command]
pub async fn list_playlists(state: State<'_, AppState>) -> AppResult<PlaylistsBundle> {
    let uid = require_uid(&state)?;
    state.db.playlists_bundle(uid)
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    thumbnail_data: Option<String>,
    thumbnail_mime: Option<String>,
) -> AppResult<CustomPlaylistSummary> {
    let uid = require_uid(&state)?;
    let thumb = match (thumbnail_data.as_deref(), thumbnail_mime.as_deref()) {
        (Some(d), Some(m)) => Some((d, m)),
        _ => None,
    };
    state.db.create_playlist(uid, &name, thumb)
}

#[tauri::command]
pub async fn update_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    name: Option<String>,
    thumbnail_data: Option<String>,
    thumbnail_mime: Option<String>,
    clear_thumbnail: Option<bool>,
) -> AppResult<CustomPlaylistSummary> {
    let uid = require_uid(&state)?;
    let thumbnail = if clear_thumbnail.unwrap_or(false) {
        Some(None)
    } else {
        match (thumbnail_data.as_deref(), thumbnail_mime.as_deref()) {
            (Some(d), Some(m)) => Some(Some((d, m))),
            _ => None,
        }
    };
    state
        .db
        .update_playlist(playlist_id, uid, name.as_deref(), thumbnail)
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, playlist_id: i64) -> AppResult<()> {
    let uid = require_uid(&state)?;
    state.db.delete_playlist(playlist_id, uid)
}

#[tauri::command]
pub async fn get_playlist_thumbnail(
    state: State<'_, AppState>,
    playlist_id: i64,
) -> AppResult<Option<String>> {
    let uid = require_uid(&state)?;
    let thumb = state.db.playlist_thumbnail(playlist_id, uid)?;
    Ok(thumb.map(|(data, mime)| format!("data:{mime};base64,{data}")))
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> AppResult<()> {
    let uid = require_uid(&state)?;
    state.db.add_track_to_playlist(playlist_id, track_id, uid)
}

#[tauri::command]
pub async fn remove_track_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> AppResult<()> {
    let uid = require_uid(&state)?;
    state
        .db
        .remove_track_from_playlist(playlist_id, track_id, uid)
}

#[tauri::command]
pub async fn toggle_like(state: State<'_, AppState>, track_id: i64) -> AppResult<Vec<i64>> {
    let uid = require_uid(&state)?;
    state.db.toggle_like(track_id, uid)
}
