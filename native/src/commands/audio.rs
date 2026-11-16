use crate::runtime::AppHost;
use crate::{
    audio::{Capabilities, Control, Request, Snapshot},
    state::AppState,
};
pub async fn native_player_command(
    state: &AppState,
    app: AppHost,
    command: crate::audio::session::Command,
) -> Result<Snapshot, String> {
    state.audio.command(app, Control::from(command)).await
}
pub async fn native_audio_capabilities(
    state: &AppState,
    app: AppHost,
) -> Result<Capabilities, String> {
    Ok(Capabilities {
        available: state
            .audio
            .command(app, Control::Capabilities)
            .await
            .is_ok(),
        streaming: true,
        codecs: vec!["mp3", "aac", "alac", "flac", "vorbis", "pcm"],
    })
}
pub fn native_audio_snapshot(state: &AppState) -> Snapshot {
    state.audio.snapshot()
}
pub async fn native_audio_load(
    state: &AppState,
    app: AppHost,
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
pub async fn native_audio_unload(state: &AppState, app: AppHost) -> Result<Snapshot, String> {
    state
        .audio
        .command(app, Control::from(crate::audio::session::Command::Clear))
        .await
}
pub async fn native_audio_play(state: &AppState, app: AppHost) -> Result<Snapshot, String> {
    state
        .audio
        .command(
            app,
            Control::from(crate::audio::session::Command::Playing { playing: true }),
        )
        .await
}
pub async fn native_audio_pause(state: &AppState, app: AppHost) -> Result<Snapshot, String> {
    state
        .audio
        .command(
            app,
            Control::from(crate::audio::session::Command::Playing { playing: false }),
        )
        .await
}
pub async fn native_audio_seek(
    state: &AppState,
    app: AppHost,
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
pub async fn native_audio_set_volume(
    state: &AppState,
    app: AppHost,
    percent: f64,
) -> Result<Snapshot, String> {
    if !percent.is_finite() {
        return Err("invalid volume".into());
    }
    state.audio.command(app, Control::Volume(percent)).await
}
