//! Authentication flows: phone + 2FA and QR login (raw `auth.ExportLoginToken`).

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use grammers_client::client::PasswordToken;
use grammers_client::SignInError;
use grammers_mtsender::InvocationError;
use grammers_session::Session;
use grammers_tl_types as tl;
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// The logged-in user, as surfaced to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub id: i64,
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}

/// Outcome of a phone sign-in step.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AuthOutcome {
    Authorized { user: AuthUser },
    PasswordRequired { hint: Option<String> },
}

/// Outcome of a QR export/poll step.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum QrOutcome {
    Waiting { url: String, expires: i32 },
    PasswordRequired { hint: Option<String> },
    Authorized { user: AuthUser },
}

/// Fetches the raw self `User` via `users.GetUsers(InputUserSelf)`.
pub async fn fetch_self_raw(client: &grammers_client::Client) -> AppResult<tl::types::User> {
    let users = client
        .invoke(&tl::functions::users::GetUsers {
            id: vec![tl::enums::InputUser::UserSelf],
        })
        .await?;

    for user in users {
        if let tl::enums::User::User(u) = user {
            return Ok(u);
        }
    }
    Err(AppError::msg("failed to load your Telegram profile"))
}

/// Fetches the current account's profile via `users.GetUsers(InputUserSelf)`.
pub async fn fetch_self(client: &grammers_client::Client) -> AppResult<AuthUser> {
    let u = fetch_self_raw(client).await?;
    Ok(AuthUser {
        id: u.id,
        first_name: u.first_name.unwrap_or_default(),
        last_name: u.last_name,
        username: u.username,
        phone: u.phone,
    })
}

/// Persists the session and profile after any successful login.
async fn finalize(state: &AppState) -> AppResult<AuthUser> {
    let user = fetch_self(&state.client).await?;
    state.persist_session()?;
    state.db.save_profile(
        user.id,
        &user.first_name,
        user.last_name.as_deref(),
        user.username.as_deref(),
        user.phone.as_deref(),
    )?;

    let mut pending = state.pending.lock().await;
    *pending = Default::default();
    Ok(user)
}

// ---- phone ---------------------------------------------------------------

pub async fn phone_send_code(state: &AppState, phone: &str) -> AppResult<()> {
    // Drop leftover QR / prior phone / 2FA tokens so sendCode isn't fighting
    // an interrupted auth attempt (common after leaving QR or the code step).
    {
        let mut pending = state.pending.lock().await;
        *pending = Default::default();
    }

    let token = state
        .client
        .request_login_code(phone, &state.config.api_hash)
        .await?;
    let mut pending = state.pending.lock().await;
    pending.phone_token = Some(token);
    Ok(())
}

pub async fn phone_sign_in(state: &AppState, code: &str) -> AppResult<AuthOutcome> {
    // `LoginToken` isn't `Clone`, so take it out; put it back if the code was
    // simply wrong so the user can retry without re-requesting a code.
    let token = {
        let mut pending = state.pending.lock().await;
        pending
            .phone_token
            .take()
            .ok_or_else(|| AppError::msg("no login in progress; request a code first"))?
    };

    match state.client.sign_in(&token, code).await {
        Ok(_) => Ok(AuthOutcome::Authorized {
            user: finalize(state).await?,
        }),
        Err(SignInError::PasswordRequired(password_token)) => {
            let hint = password_token.hint().map(str::to_owned);
            let mut pending = state.pending.lock().await;
            pending.password_token = Some(password_token);
            Ok(AuthOutcome::PasswordRequired { hint })
        }
        Err(SignInError::InvalidCode) => {
            let mut pending = state.pending.lock().await;
            pending.phone_token = Some(token);
            Err(AppError::msg("the code you entered is invalid"))
        }
        Err(e) => Err(AppError::Telegram(e.to_string())),
    }
}

pub async fn check_password(state: &AppState, password: &str) -> AppResult<AuthUser> {
    let token = {
        let mut pending = state.pending.lock().await;
        pending
            .password_token
            .take()
            .ok_or_else(|| AppError::msg("no password step in progress"))?
    };

    match state.client.check_password(token, password.trim()).await {
        Ok(_) => finalize(state).await,
        Err(SignInError::InvalidPassword(_)) => Err(AppError::msg("incorrect password")),
        Err(e) => Err(AppError::Telegram(e.to_string())),
    }
}

// ---- QR ------------------------------------------------------------------

/// Runs one `auth.ExportLoginToken` round. Used both to start the QR flow and
/// to poll it: while unscanned it returns a `Waiting` token URL; once scanned
/// it resolves to `Authorized` (following any DC migration) or, when 2FA is
/// enabled, `PasswordRequired`.
pub async fn qr_export(state: &AppState) -> AppResult<QrOutcome> {
    let request = tl::functions::auth::ExportLoginToken {
        api_id: state.config.api_id,
        api_hash: state.config.api_hash.clone(),
        except_ids: Vec::new(),
    };

    let result = match state.client.invoke(&request).await {
        Ok(r) => r,
        Err(InvocationError::Rpc(e)) if e.name.contains("SESSION_PASSWORD_NEEDED") => {
            return qr_password_required(state).await;
        }
        Err(e) => return Err(e.into()),
    };

    handle_login_token(state, result).await
}

/// Submits the 2FA password for the QR flow.
pub async fn qr_check_password(state: &AppState, password: &str) -> AppResult<AuthUser> {
    check_password(state, password).await
}

async fn qr_password_required(state: &AppState) -> AppResult<QrOutcome> {
    let password = state
        .client
        .invoke(&tl::functions::account::GetPassword {})
        .await?;
    let tl::enums::account::Password::Password(pw) = password;
    let token = PasswordToken::new(pw);
    let hint = token.hint().map(str::to_owned);

    let mut pending = state.pending.lock().await;
    pending.password_token = Some(token);
    Ok(QrOutcome::PasswordRequired { hint })
}

async fn handle_login_token(
    state: &AppState,
    token: tl::enums::auth::LoginToken,
) -> AppResult<QrOutcome> {
    match token {
        tl::enums::auth::LoginToken::Token(t) => {
            let url = format!("tg://login?token={}", B64URL.encode(&t.token));
            let mut pending = state.pending.lock().await;
            pending.qr_token = Some(t.token);
            Ok(QrOutcome::Waiting {
                url,
                expires: t.expires,
            })
        }
        tl::enums::auth::LoginToken::MigrateTo(m) => {
            state.session.set_home_dc_id(m.dc_id).await;
            let imported = state
                .client
                .invoke_in_dc(
                    m.dc_id,
                    &tl::functions::auth::ImportLoginToken { token: m.token },
                )
                .await?;
            // After import the token should resolve to success.
            match imported {
                tl::enums::auth::LoginToken::Success(_) => Ok(QrOutcome::Authorized {
                    user: finalize(state).await?,
                }),
                other => Box::pin(handle_login_token(state, other)).await,
            }
        }
        tl::enums::auth::LoginToken::Success(_) => Ok(QrOutcome::Authorized {
            user: finalize(state).await?,
        }),
    }
}

// ---- session lifecycle ---------------------------------------------------

pub async fn logout(state: &AppState) -> AppResult<()> {
    // Best-effort remote sign-out; local library data stays available for the
    // same Telegram user on this device after they sign in again.
    let _ = state.client.sign_out().await;
    if let Ok(Some(profile)) = state.db.load_profile() {
        state.db.clear_active_profile(profile.tg_user_id)?;
    }
    crate::session::clear(&state.data_dir)?;
    let mut pending = state.pending.lock().await;
    *pending = Default::default();
    Ok(())
}
