use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::state::AppState;
use crate::telegram::auth::{
    self as telegram_auth, AuthOutcome, AuthStatus, AuthUser, PhoneSendCodeOutcome, QrOutcome,
};

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> AppResult<AuthStatus> {
    // Local-only: never touch the network. Offline UI hydrates from SQLite + session.enc.
    if !crate::session::exists(&state.data_dir) {
        return Ok(AuthStatus {
            authorized: false,
            user: None,
        });
    }
    let Some(profile) = state.db.load_profile()? else {
        return Ok(AuthStatus {
            authorized: false,
            user: None,
        });
    };
    Ok(AuthStatus {
        authorized: true,
        user: Some(AuthUser {
            id: profile.tg_user_id,
            first_name: profile.first_name,
            last_name: profile.last_name,
            username: profile.username,
            phone: profile.phone,
        }),
    })
}

#[tauri::command]
pub async fn refresh_auth(state: State<'_, AppState>, app: AppHandle) -> AppResult<AuthStatus> {
    telegram_auth::refresh_auth(&state, &app).await
}

#[tauri::command]
pub async fn phone_send_code(
    state: State<'_, AppState>,
    phone: String,
) -> AppResult<PhoneSendCodeOutcome> {
    state.ensure_client().await?;
    telegram_auth::phone_send_code(&state, phone.trim()).await
}

#[tauri::command]
pub async fn phone_sign_in(state: State<'_, AppState>, code: String) -> AppResult<AuthOutcome> {
    telegram_auth::phone_sign_in(&state, code.trim()).await
}

#[tauri::command]
pub async fn phone_check_password(
    state: State<'_, AppState>,
    password: String,
) -> AppResult<AuthUser> {
    telegram_auth::check_password(&state, &password).await
}

#[tauri::command]
pub async fn qr_start(state: State<'_, AppState>) -> AppResult<QrOutcome> {
    state.ensure_client().await?;
    telegram_auth::qr_export(&state).await
}

#[tauri::command]
pub async fn qr_poll(state: State<'_, AppState>) -> AppResult<QrOutcome> {
    state.ensure_client().await?;
    telegram_auth::qr_export(&state).await
}

#[tauri::command]
pub async fn qr_restart(state: State<'_, AppState>) -> AppResult<QrOutcome> {
    telegram_auth::qr_restart(&state).await
}

#[tauri::command]
pub async fn qr_check_password(
    state: State<'_, AppState>,
    password: String,
) -> AppResult<AuthUser> {
    telegram_auth::qr_check_password(&state, &password).await
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> AppResult<()> {
    telegram_auth::logout(&state).await
}
