use crate::{
    audio::{Capabilities, Control, Request, Snapshot},
    state::AppState,
};
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn native_player_command(
    state: State<'_, AppState>,
    app: AppHandle,
    command: crate::audio::session::Command,
) -> Result<Snapshot, String> {
    state.audio.command(app, Control::from(command)).await
}
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
    // Compatibility entry point still goes through the authoritative queue.
    let profile = state
        .db
        .load_profile()
        .map_err(|e| e.to_string())?
        .ok_or("Not signed in")?;
    let track = state
        .db
        .track_by_id(request.track_id, profile.tg_user_id)
        .map_err(|e| e.to_string())?
        .ok_or("Track unavailable")?;
    state
        .audio
        .command(
            app,
            Control::from(crate::audio::session::Command::SetQueue {
                queue: crate::audio::session::Queue {
                    tracks: vec![track],
                    cursor: 0,
                    ..Default::default()
                },
                play: false,
            }),
        )
        .await
}
#[tauri::command]
pub async fn native_audio_unload(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Snapshot, String> {
    state
        .audio
        .command(app, Control::from(crate::audio::session::Command::Clear))
        .await
}
#[tauri::command]
pub async fn native_audio_play(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Snapshot, String> {
    state
        .audio
        .command(
            app,
            Control::from(crate::audio::session::Command::Playing { playing: true }),
        )
        .await
}
#[tauri::command]
pub async fn native_audio_pause(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Snapshot, String> {
    state
        .audio
        .command(
            app,
            Control::from(crate::audio::session::Command::Playing { playing: false }),
        )
        .await
}
#[tauri::command]
pub async fn native_audio_seek(
    state: State<'_, AppState>,
    app: AppHandle,
    seconds: f64,
    attempt_id: Option<String>,
) -> Result<Snapshot, String> {
    if !seconds.is_finite() {
        return Err("invalid seek".into());
    }
    state
        .audio
        .command(app, Control::SeekAttempt(seconds, attempt_id))
        .await
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
