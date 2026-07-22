//! Authentication flows: phone + 2FA and QR login via ferogram.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use ferogram::{Client, InvocationError, PasswordToken, SendCodeOutcome, SignInError};
use ferogram::tl;
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

/// Outcome of `phone_send_code` (code pending, or session already restored).
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PhoneSendCodeOutcome {
    CodeSent,
    Authorized { user: AuthUser },
}

/// Outcome of a QR export/poll step.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum QrOutcome {
    Waiting { url: String, expires: i32 },
    PasswordRequired { hint: Option<String> },
    Authorized { user: AuthUser },
}

/// Fetches the raw self `User` via ferogram's `get_me`.
pub async fn fetch_self_raw(client: &Client) -> AppResult<tl::types::User> {
    Ok(client.get_me().await?)
}

/// Fetches the current account's profile.
pub async fn fetch_self(client: &Client) -> AppResult<AuthUser> {
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
    let client = state.client().await?;
    let user = fetch_self(&client).await?;
    state.persist_session().await?;
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

pub async fn phone_send_code(state: &AppState, phone: &str) -> AppResult<PhoneSendCodeOutcome> {
    // Drop leftover QR / prior phone / 2FA tokens so sendCode isn't fighting
    // an interrupted auth attempt (common after leaving QR or the code step).
    {
        let mut pending = state.pending.lock().await;
        *pending = Default::default();
    }

    let client = state.client().await?;
    match client.request_login_code(phone).await? {
        SendCodeOutcome::CodeRequired(token) => {
            let mut pending = state.pending.lock().await;
            pending.phone_token = Some(token);
            Ok(PhoneSendCodeOutcome::CodeSent)
        }
        SendCodeOutcome::AlreadyAuthorized(_) => {
            // Logout-token fast path: session is already signed in.
            let user = finalize(state).await?;
            Ok(PhoneSendCodeOutcome::Authorized { user })
        }
    }
}

pub async fn phone_sign_in(state: &AppState, code: &str) -> AppResult<AuthOutcome> {
    let token = {
        let mut pending = state.pending.lock().await;
        pending
            .phone_token
            .take()
            .ok_or_else(|| AppError::msg("no login in progress; request a code first"))?
    };

    let client = state.client().await?;
    match client.sign_in(&token, code).await {
        Ok(_) => Ok(AuthOutcome::Authorized {
            user: finalize(state).await?,
        }),
        Err(SignInError::PasswordRequired(password_token)) => {
            let hint = password_token.hint().map(str::to_owned);
            let mut pending = state.pending.lock().await;
            pending.password_token = Some(*password_token);
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

    let client = state.client().await?;
    match client
        .check_password(token, password.trim().as_bytes())
        .await
    {
        Ok(_) => finalize(state).await,
        Err(InvocationError::Rpc(e)) if e.name.contains("PASSWORD") => {
            Err(AppError::msg("incorrect password"))
        }
        Err(e) => Err(AppError::Telegram(e.to_string())),
    }
}

// ---- QR ------------------------------------------------------------------

/// Runs one QR login round. Used both to start the flow and to poll it.
pub async fn qr_export(state: &AppState) -> AppResult<QrOutcome> {
    let client = state.client().await?;
    match client.export_login_token().await {
        Ok((token, _expires)) if token.is_empty() => Ok(QrOutcome::Authorized {
            user: finalize(state).await?,
        }),
        Ok((token, expires)) => {
            let url = format!("tg://login?token={}", B64URL.encode(&token));
            let mut pending = state.pending.lock().await;
            pending.qr_token = Some(token);
            Ok(QrOutcome::Waiting { url, expires })
        }
        Err(InvocationError::Rpc(e)) if e.name.contains("SESSION_PASSWORD_NEEDED") => {
            qr_password_required(state).await
        }
        Err(e) => Err(e.into()),
    }
}

/// Submits the 2FA password for the QR flow.
pub async fn qr_check_password(state: &AppState, password: &str) -> AppResult<AuthUser> {
    check_password(state, password).await
}

async fn qr_password_required(state: &AppState) -> AppResult<QrOutcome> {
    let client = state.client().await?;
    let password = client
        .invoke(&tl::functions::account::GetPassword {})
        .await?;
    let tl::enums::account::Password::Password(pw) = password;
    let token = PasswordToken { password: pw };
    let hint = token.hint().map(str::to_owned);

    let mut pending = state.pending.lock().await;
    pending.password_token = Some(token);
    Ok(QrOutcome::PasswordRequired { hint })
}

// ---- session lifecycle ---------------------------------------------------

pub async fn logout(state: &AppState) -> AppResult<()> {
    // Best-effort remote sign-out; local library data stays available for the
    // same Telegram user on this device after they sign in again.
    if let Ok(client) = state.client().await {
        let _ = client.sign_out().await;
    }
    if let Ok(Some(profile)) = state.db.load_profile() {
        state.db.clear_active_profile(profile.tg_user_id)?;
    }
    crate::session::clear(&state.data_dir)?;
    let mut pending = state.pending.lock().await;
    *pending = Default::default();
    Ok(())
}
