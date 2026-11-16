use crate::error::AppResult;
use crate::lastfm::models::LastFmStatus;
use crate::runtime::AppHost;
use crate::state::AppState;

pub async fn get_lastfm_status(state: &AppState) -> AppResult<LastFmStatus> {
    state.lastfm.status(&state.db).await
}

pub async fn start_lastfm_auth(state: &AppState, app: AppHost) -> AppResult<LastFmStatus> {
    state.lastfm.start_auth(&state.db, &app).await
}

pub async fn complete_lastfm_auth(state: &AppState, app: AppHost) -> AppResult<LastFmStatus> {
    state.lastfm.complete_auth(&state.db, &app).await
}

pub async fn cancel_lastfm_auth(state: &AppState, app: AppHost) -> AppResult<LastFmStatus> {
    state.lastfm.cancel_auth(&state.db, &app).await
}

pub async fn set_lastfm_enabled(
    state: &AppState,
    app: AppHost,
    enabled: bool,
) -> AppResult<LastFmStatus> {
    state.lastfm.set_enabled(&state.db, &app, enabled).await
}

pub async fn disconnect_lastfm(
    state: &AppState,
    app: AppHost,
    pending_action: Option<String>,
) -> AppResult<LastFmStatus> {
    state
        .lastfm
        .disconnect(&state.db, &app, pending_action.as_deref())
        .await
}

pub async fn open_lastfm_profile(state: &AppState, app: AppHost) -> AppResult<()> {
    state.lastfm.open_profile(&app).await
}

pub async fn flush_lastfm_queue(state: &AppState) -> AppResult<()> {
    state.lastfm.wake();
    Ok(())
}
