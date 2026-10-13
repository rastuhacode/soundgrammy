use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::lastfm::models::LastFmStatus;
use crate::state::AppState;

#[tauri::command]
pub async fn get_lastfm_status(state: State<'_, AppState>) -> AppResult<LastFmStatus> {
    state.lastfm.status(&state.db).await
}

#[tauri::command]
pub async fn start_lastfm_auth(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<LastFmStatus> {
    state.lastfm.start_auth(&state.db, &app).await
}

#[tauri::command]
pub async fn complete_lastfm_auth(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<LastFmStatus> {
    state.lastfm.complete_auth(&state.db, &app).await
}

#[tauri::command]
pub async fn cancel_lastfm_auth(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<LastFmStatus> {
    state.lastfm.cancel_auth(&state.db, &app).await
}

#[tauri::command]
pub async fn set_lastfm_enabled(
    state: State<'_, AppState>,
    app: AppHandle,
    enabled: bool,
) -> AppResult<LastFmStatus> {
    state.lastfm.set_enabled(&state.db, &app, enabled).await
}

#[tauri::command]
pub async fn disconnect_lastfm(
    state: State<'_, AppState>,
    app: AppHandle,
    pending_action: Option<String>,
) -> AppResult<LastFmStatus> {
    state
        .lastfm
        .disconnect(&state.db, &app, pending_action.as_deref())
        .await
}

#[tauri::command]
pub async fn open_lastfm_profile(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    state.lastfm.open_profile(&app).await
}

#[tauri::command]
pub async fn flush_lastfm_queue(state: State<'_, AppState>) -> AppResult<()> {
    state.lastfm.wake();
    Ok(())
}
