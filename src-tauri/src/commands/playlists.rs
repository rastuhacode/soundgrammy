use tauri::State;

use super::require_uid;
use crate::db::{CustomPlaylistSummary, LikedPlaylist, PlaylistsBundle};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn list_playlists(state: State<'_, AppState>) -> AppResult<PlaylistsBundle> {
    let uid = require_uid(&state)?;
    state.db.playlists_bundle(uid)
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<CustomPlaylistSummary> {
    let uid = require_uid(&state)?;
    state.db.create_playlist(uid, &name)
}

#[tauri::command]
pub async fn update_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    name: Option<String>,
) -> AppResult<CustomPlaylistSummary> {
    let uid = require_uid(&state)?;
    state.db.update_playlist(playlist_id, uid, name.as_deref())
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, playlist_id: i64) -> AppResult<()> {
    let uid = require_uid(&state)?;
    state.db.delete_playlist(playlist_id, uid)
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    state.db.add_track_to_playlist(playlist_id, track_id, uid)
}

#[tauri::command]
pub async fn add_tracks_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    state
        .db
        .add_tracks_to_playlist(playlist_id, &track_ids, uid)
}

#[tauri::command]
pub async fn remove_track_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    position: i64,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    state
        .db
        .remove_track_from_playlist(playlist_id, position, uid)
}

#[tauri::command]
pub async fn reorder_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    state
        .db
        .reorder_playlist_tracks(playlist_id, &track_ids, uid)
}

#[tauri::command]
pub async fn toggle_like(state: State<'_, AppState>, track_id: i64) -> AppResult<LikedPlaylist> {
    let uid = require_uid(&state)?;
    state.db.toggle_like(track_id, uid)
}

/// Write a SoundGrammy playlist recipe JSON to `path` (Liked or custom).
#[tauri::command]
pub async fn export_playlist_json(
    state: State<'_, AppState>,
    source: crate::playlist_recipe::PlaylistRecipeSource,
    path: String,
) -> AppResult<()> {
    crate::playlist_recipe::export_playlist_json(&state, source, path)
}

/// Analyze a playlist recipe JSON without creating a playlist.
#[tauri::command]
pub async fn analyze_playlist_json(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<crate::playlist_recipe::PlaylistImportPreview> {
    crate::playlist_recipe::analyze_playlist_json(&state, path)
}

/// Import a playlist recipe JSON from `path` as a new custom playlist.
#[tauri::command]
pub async fn import_playlist_json(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
) -> AppResult<crate::playlist_recipe::PlaylistImportResult> {
    crate::playlist_recipe::import_playlist_json(&state, path, name)
}
