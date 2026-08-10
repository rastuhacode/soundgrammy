use tauri::State;

use crate::db::{ListenEndResult, TrackListenStats, SETTING_LISTEN_STATS_ENABLED};
use crate::error::{AppError, AppResult};
use crate::listen_stats::EndReason;
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
    enabled: bool,
) -> AppResult<()> {
    state.db.set_setting(
        SETTING_LISTEN_STATS_ENABLED,
        if enabled { "true" } else { "false" },
    )
}

#[tauri::command]
pub async fn record_listen_start(state: State<'_, AppState>, track_id: i64) -> AppResult<()> {
    if !listen_statistics_enabled(&state)? {
        return Ok(());
    }
    state.db.record_attempt_start(track_id)
}

#[tauri::command]
pub async fn record_listen_end(
    state: State<'_, AppState>,
    track_id: i64,
    listened_ms: i64,
    duration_ms: Option<i64>,
    end_reason: String,
) -> AppResult<Option<ListenEndResult>> {
    if !listen_statistics_enabled(&state)? {
        return Ok(None);
    }
    let reason = EndReason::parse(&end_reason)
        .ok_or_else(|| AppError::msg(format!("invalid end_reason: {end_reason}")))?;
    state
        .db
        .record_attempt_end(track_id, listened_ms, duration_ms, reason)
        .map(Some)
}

#[tauri::command]
pub async fn get_track_listen_stats(
    state: State<'_, AppState>,
    track_id: i64,
) -> AppResult<Option<TrackListenStats>> {
    state.db.track_listen_stats(track_id)
}

#[tauri::command]
pub async fn list_listen_stats(state: State<'_, AppState>) -> AppResult<Vec<TrackListenStats>> {
    state.db.all_listen_stats()
}

#[tauri::command]
pub async fn rebuild_listen_stats(state: State<'_, AppState>) -> AppResult<()> {
    state.db.rebuild_listen_stats()
}

#[tauri::command]
pub async fn clear_listen_statistics(state: State<'_, AppState>) -> AppResult<()> {
    state.db.clear_listen_stats()
}
