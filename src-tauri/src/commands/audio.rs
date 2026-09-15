use crate::{
    audio::{Capabilities, Control, Request, Snapshot},
    state::AppState,
};
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn native_audio_capabilities(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Capabilities, String> {
    Ok(Capabilities {
        available: state
            .audio
            .command(app, Control::Capabilities)
            .await
            .is_ok(),
        streaming: true,
    })
}
#[tauri::command]
pub fn native_audio_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.audio.snapshot()
}
#[tauri::command]
pub async fn native_audio_load(
    state: State<'_, AppState>,
    app: AppHandle,
    request: Request,
) -> Result<Snapshot, String> {
    if request.attempt_id.is_empty() || request.attempt_id.len() > 256 {
        return Err("invalid attempt".into());
    }
    state.audio.command(app, Control::Load(request)).await
}
#[tauri::command]
pub async fn native_audio_unload(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Snapshot, String> {
    state.audio.command(app, Control::Unload).await
}
#[tauri::command]
pub async fn native_audio_play(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Snapshot, String> {
    state.audio.command(app, Control::Play).await
}
#[tauri::command]
pub async fn native_audio_pause(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Snapshot, String> {
    state.audio.command(app, Control::Pause).await
}
#[tauri::command]
pub async fn native_audio_seek(
    state: State<'_, AppState>,
    app: AppHandle,
    seconds: f64,
) -> Result<Snapshot, String> {
    if !seconds.is_finite() {
        return Err("invalid seek".into());
    }
    state.audio.command(app, Control::Seek(seconds)).await
}
#[tauri::command]
pub async fn native_audio_set_volume(
    state: State<'_, AppState>,
    app: AppHandle,
    percent: f64,
) -> Result<Snapshot, String> {
    if !percent.is_finite() {
        return Err("invalid volume".into());
    }
    state.audio.command(app, Control::Volume(percent)).await
}
