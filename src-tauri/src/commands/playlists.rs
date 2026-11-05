use tauri::{AppHandle, State};

use super::require_uid;
use crate::db::{CustomPlaylistSummary, LikedPlaylist, PlaylistsBundle};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn list_playlists(state: State<'_, AppState>) -> AppResult<PlaylistsBundle> {
    let uid = require_uid(&state)?;
    state.db.blocking(move |db| db.playlists_bundle(uid)).await
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    track_ids: Option<Vec<i64>>,
) -> AppResult<CustomPlaylistSummary> {
    let uid = require_uid(&state)?;
    state
        .db
        .blocking(move |db| {
            db.create_playlist_with_tracks(uid, &name, &track_ids.unwrap_or_default())
        })
        .await
}

#[tauri::command]
pub async fn update_playlist(
    state: State<'_, AppState>,
    app: AppHandle,
    playlist_id: i64,
    name: Option<String>,
) -> AppResult<CustomPlaylistSummary> {
    let uid = require_uid(&state)?;
    let result = state
        .db
        .blocking(move |db| db.update_playlist(playlist_id, uid, name.as_deref()))
        .await?;
    state
        .audio
        .reconcile_playlist_metadata(app, uid, playlist_id, Some(result.name.clone()))
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn delete_playlist(
    state: State<'_, AppState>,
    app: AppHandle,
    playlist_id: i64,
) -> AppResult<()> {
    let uid = require_uid(&state)?;
    state
        .db
        .blocking(move |db| db.delete_playlist(playlist_id, uid))
        .await?;
    state
        .audio
        .reconcile_playlist_metadata(app, uid, playlist_id, None)
        .await
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, AppState>,
    app: AppHandle,
    playlist_id: i64,
    track_id: i64,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    let result = state
        .db
        .blocking(move |db| db.add_track_to_playlist(playlist_id, track_id, uid))
        .await?;
    state
        .audio
        .reconcile_playlist(app, uid, playlist_id, None, None)
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn add_tracks_to_playlist(
    state: State<'_, AppState>,
    app: AppHandle,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    let result = state
        .db
        .blocking(move |db| db.add_tracks_to_playlist(playlist_id, &track_ids, uid))
        .await?;
    state
        .audio
        .reconcile_playlist(app, uid, playlist_id, None, None)
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn remove_tracks_from_playlist(
    state: State<'_, AppState>,
    app: AppHandle,
    playlist_id: i64,
    positions: Vec<i64>,
    expected_track_ids: Vec<i64>,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    let removed_positions = positions.clone();
    let expected_len = expected_track_ids.len();
    let result = state
        .db
        .blocking(move |db| {
            db.remove_playlist_tracks(playlist_id, &positions, &expected_track_ids, uid)
        })
        .await?;
    let removed: std::collections::HashSet<_> = removed_positions.into_iter().collect();
    let mut next = 0;
    let map = (0..expected_len)
        .map(|index| {
            if removed.contains(&(index as i64)) {
                None
            } else {
                let value = next;
                next += 1;
                Some(value)
            }
        })
        .collect();
    state
        .audio
        .reconcile_playlist(app, uid, playlist_id, None, Some(map))
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn reorder_playlist_tracks(
    state: State<'_, AppState>,
    app: AppHandle,
    playlist_id: i64,
    track_ids: Vec<i64>,
    expected_track_ids: Vec<i64>,
    move_indices: Option<[i64; 2]>,
) -> AppResult<String> {
    let uid = require_uid(&state)?;
    let result = state
        .db
        .blocking(move |db| {
            db.reorder_playlist_tracks_guarded(
                playlist_id,
                &track_ids,
                &expected_track_ids,
                move_indices,
                uid,
            )
        })
        .await?;
    state
        .audio
        .reconcile_playlist(app, uid, playlist_id, move_indices, None)
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn toggle_like(
    state: State<'_, AppState>,
    app: AppHandle,
    track_id: i64,
) -> AppResult<LikedPlaylist> {
    let uid = require_uid(&state)?;
    let result = state
        .db
        .blocking(move |db| db.toggle_like(track_id, uid))
        .await?;
    state
        .audio
        .reconcile_playlist(app, uid, result.id, None, None)
        .await?;
    Ok(result)
}

/// Write a SoundGrammy playlist recipe JSON to `path` (Liked or custom).
#[tauri::command]
pub async fn export_playlist_json(
    state: State<'_, AppState>,
    source: crate::playlist_recipe::PlaylistRecipeSource,
    path: String,
) -> AppResult<()> {
    state
        .db
        .blocking(move |db| crate::playlist_recipe::export_playlist_json(db, source, path))
        .await
}

/// Analyze a playlist recipe JSON without creating a playlist.
#[tauri::command]
pub async fn analyze_playlist_json(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<crate::playlist_recipe::PlaylistImportPreview> {
    state
        .db
        .blocking(move |db| crate::playlist_recipe::analyze_playlist_json(db, path))
        .await
}

/// Import a playlist recipe JSON from `path` as a new custom playlist.
#[tauri::command]
pub async fn import_playlist_json(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
) -> AppResult<crate::playlist_recipe::PlaylistImportResult> {
    state
        .db
        .blocking(move |db| crate::playlist_recipe::import_playlist_json(db, path, name))
        .await
}
