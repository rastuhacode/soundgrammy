use tauri::State;

use crate::db::{TrackListenStats, SETTING_LISTEN_STATS_ENABLED};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

fn listen_statistics_enabled(state: &AppState) -> AppResult<bool> {
    Ok(state
        .db
        .get_setting(SETTING_LISTEN_STATS_ENABLED)?
        .as_deref()
        != Some("false"))
}

#[tauri::command]
pub async fn get_listen_statistics_enabled(state: State<'_, AppState>) -> AppResult<bool> {
    listen_statistics_enabled(&state)
}

#[tauri::command]
pub async fn set_listen_statistics_enabled(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    enabled: bool,
) -> AppResult<()> {
    state
        .audio
        .command(
            app,
            crate::audio::Control::ListenSettings {
                enabled: Some(enabled),
                clear: false,
            },
        )
        .await
        .map(|_| ())
        .map_err(AppError::msg)
}

#[tauri::command]
pub async fn get_track_listen_stats(
    state: State<'_, AppState>,
    track_id: i64,
) -> AppResult<Option<TrackListenStats>> {
    let uid = super::require_uid(&state)?;
    if state.db.track_by_id(track_id, uid)?.is_none() {
        return Err(AppError::msg("Track not found"));
    }
    state.db.track_listen_stats(track_id)
}

#[tauri::command]
pub async fn list_listen_stats(state: State<'_, AppState>) -> AppResult<Vec<TrackListenStats>> {
    state
        .db
        .blocking(|db| db.listen_stats_for_user(db.active_uid()?))
        .await
}

#[tauri::command]
pub async fn rebuild_listen_stats(state: State<'_, AppState>) -> AppResult<()> {
    state.db.blocking(|db| db.rebuild_listen_stats()).await
}

#[tauri::command]
pub async fn clear_listen_statistics(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> AppResult<()> {
    state
        .audio
        .command(
            app,
            crate::audio::Control::ListenSettings {
                enabled: None,
                clear: true,
            },
        )
        .await
        .map(|_| ())
        .map_err(AppError::msg)
}
