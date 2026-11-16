use crate::error::AppResult;
use crate::runtime::AppHost;
use crate::state::AppState;
use crate::telegram::auth::{
    self as telegram_auth, AuthOutcome, AuthStatus, AuthUser, PhoneSendCodeOutcome, QrOutcome,
};

pub async fn auth_status(state: &AppState) -> AppResult<AuthStatus> {
    // Local-only: keychain/disk verification runs on a blocking worker, never Telegram.
    let directory = state.data_dir.clone();
    let local = state
        .db
        .blocking(move |_| Ok(crate::session::local_state(&directory)))
        .await?;
    if matches!(
        local,
        crate::session::LocalSessionState::Missing | crate::session::LocalSessionState::Corrupt
    ) {
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

pub async fn refresh_auth(state: &AppState, app: AppHost) -> AppResult<AuthStatus> {
    telegram_auth::refresh_auth(state, &app).await
}

pub async fn phone_send_code(state: &AppState, phone: String) -> AppResult<PhoneSendCodeOutcome> {
    let directory = state.data_dir.clone();
    state
        .db
        .blocking(move |_| crate::session::prepare_new_login(&directory))
        .await?;
    state.ensure_client().await?;
    telegram_auth::phone_send_code(state, phone.trim()).await
}

pub async fn phone_sign_in(state: &AppState, code: String) -> AppResult<AuthOutcome> {
    telegram_auth::phone_sign_in(state, code.trim()).await
}

pub async fn phone_check_password(state: &AppState, password: String) -> AppResult<AuthUser> {
    telegram_auth::check_password(state, &password).await
}

pub async fn qr_start(state: &AppState) -> AppResult<QrOutcome> {
    let directory = state.data_dir.clone();
    state
        .db
        .blocking(move |_| crate::session::prepare_new_login(&directory))
        .await?;
    state.ensure_client().await?;
    telegram_auth::qr_export(state).await
}

pub async fn qr_poll(state: &AppState) -> AppResult<QrOutcome> {
    state.ensure_client().await?;
    telegram_auth::qr_export(state).await
}

pub async fn qr_restart(state: &AppState) -> AppResult<QrOutcome> {
    telegram_auth::qr_restart(state).await
}

pub async fn qr_check_password(state: &AppState, password: String) -> AppResult<AuthUser> {
    telegram_auth::qr_check_password(state, &password).await
}

pub async fn logout(state: &AppState) -> AppResult<()> {
    telegram_auth::logout(state).await
}
