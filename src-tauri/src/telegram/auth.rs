//! Authentication flows: phone + 2FA and QR login via ferogram.

use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use ferogram::tl;
use ferogram::{Client, InvocationError, PasswordToken, SendCodeOutcome, SignInError};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::client;

/// How long background online auth refresh may wait before treating Telegram as unreachable.
const REFRESH_AUTH_TIMEOUT: Duration = Duration::from_secs(8);

/// RPC / error fragments that mean the local session is dead on the server.
const AUTH_REVOKED_MARKERS: &[&str] = &[
    "AUTH_KEY_DUPLICATED",
    "AUTH_KEY_UNREGISTERED",
    "AUTH_KEY_INVALID",
    "SESSION_REVOKED",
    "SESSION_EXPIRED",
    "USER_DEACTIVATED",
    "USER_DEACTIVATED_BAN",
];

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

/// Local or refreshed session status for the UI.
#[derive(Debug, Clone, Serialize)]
pub struct AuthStatus {
    pub authorized: bool,
    pub user: Option<AuthUser>,
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

pub(crate) fn is_auth_revoked_message(message: &str) -> bool {
    let upper = message.to_ascii_uppercase();
    AUTH_REVOKED_MARKERS
        .iter()
        .any(|marker| upper.contains(marker))
}

fn clear_local_session(state: &AppState) -> AppResult<()> {
    if let Ok(Some(profile)) = state.db.load_profile() {
        state.db.clear_active_profile(profile.tg_user_id)?;
    }
    crate::session::clear(&state.data_dir)?;
    Ok(())
}

async fn clear_local_session_and_pending(state: &AppState) -> AppResult<()> {
    state.disconnect_client().await;
    clear_local_session(state)?;
    let mut pending = state.pending.lock().await;
    *pending = Default::default();
    Ok(())
}

fn emit_auth_revoked(app: &AppHandle) {
    let _ = app.emit("auth:revoked", ());
}

/// Online check: refresh profile from Telegram, or report unreachable / revoked.
///
/// Timeouts and generic network errors do **not** clear the local session.
/// Only server-proven auth death clears local session and emits `auth:revoked`.
pub async fn refresh_auth(state: &AppState, app: &AppHandle) -> AppResult<AuthStatus> {
    if !crate::session::exists(&state.data_dir) || state.db.load_profile()?.is_none() {
        return Ok(AuthStatus {
            authorized: false,
            user: None,
        });
    }

    let result = tokio::time::timeout(REFRESH_AUTH_TIMEOUT, async {
        let client = state.ensure_client().await?;
        let authorized = client::is_authorized(&client).await?;
        if !authorized {
            return Ok::<AuthStatus, AppError>(AuthStatus {
                authorized: false,
                user: None,
            });
        }
        let user = fetch_self(&client).await?;
        state.db.save_profile(
            user.id,
            &user.first_name,
            user.last_name.as_deref(),
            user.username.as_deref(),
            user.phone.as_deref(),
        )?;
        Ok(AuthStatus {
            authorized: true,
            user: Some(user),
        })
    })
    .await;

    match result {
        Ok(Ok(status)) => {
            if !status.authorized {
                // Session file present but client reports unauthorized — treat as revoked.
                clear_local_session_and_pending(state).await?;
                emit_auth_revoked(app);
            }
            Ok(status)
        }
        Ok(Err(err)) => {
            let message = err.to_string();
            if is_auth_revoked_message(&message) {
                clear_local_session_and_pending(state).await?;
                emit_auth_revoked(app);
                return Ok(AuthStatus {
                    authorized: false,
                    user: None,
                });
            }
            // Unreachable / transient — keep local session; surface as soft error for UI.
            state.disconnect_client().await;
            Err(AppError::msg("telegram unreachable"))
        }
        Err(_elapsed) => {
            state.disconnect_client().await;
            Err(AppError::msg("telegram unreachable"))
        }
    }
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
    let retry_token = token.clone();

    let client = state.client().await?;
    match client
        .check_password(token, password.trim().as_bytes())
        .await
    {
        Ok(_) => finalize(state).await,
        Err(InvocationError::Rpc(e)) if e.name.contains("PASSWORD") => {
            // A failed attempt consumes our local token. Refresh the SRP
            // parameters so the user can retry without restarting login.
            let retry_token = load_password_token(&client).await.unwrap_or(retry_token);
            let mut pending = state.pending.lock().await;
            pending.password_token = Some(retry_token);
            Err(AppError::msg("incorrect password"))
        }
        Err(e) => Err(AppError::Telegram(e.to_string())),
    }
}

// ---- QR ------------------------------------------------------------------

async fn qr_export_once(state: &AppState, client: &Client) -> AppResult<QrOutcome> {
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

/// Runs one QR login round. Used both to start the flow and to poll it.
pub async fn qr_export(state: &AppState) -> AppResult<QrOutcome> {
    let client = state.client().await?;
    match qr_export_once(state, &client).await {
        Err(err) if is_auth_revoked_message(&err.to_string()) => {
            // Telegram has already invalidated this key. Discard both its
            // in-memory connection and persisted snapshot, then retry once
            // with a fresh unauthenticated client so QR login can recover.
            clear_local_session_and_pending(state).await?;
            let client = state.ensure_client().await?;
            qr_export_once(state, &client).await
        }
        outcome => outcome,
    }
}

/// Abandons an accepted QR challenge and starts over with a fresh auth key.
pub async fn qr_restart(state: &AppState) -> AppResult<QrOutcome> {
    state.disconnect_client().await;
    crate::session::clear(&state.data_dir)?;
    state.ensure_client().await?;
    qr_export(state).await
}

/// Submits the 2FA password for the QR flow.
pub async fn qr_check_password(state: &AppState, password: &str) -> AppResult<AuthUser> {
    check_password(state, password).await
}

async fn load_password_token(client: &Client) -> AppResult<PasswordToken> {
    let password = client
        .invoke(&tl::functions::account::GetPassword {})
        .await?;
    let tl::enums::account::Password::Password(pw) = password;
    Ok(PasswordToken { password: pw })
}

async fn qr_password_required(state: &AppState) -> AppResult<QrOutcome> {
    let client = state.client().await?;
    let token = load_password_token(&client).await?;
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
    clear_local_session_and_pending(state).await
}

#[cfg(test)]
mod tests {
    use super::is_auth_revoked_message;

    #[test]
    fn treats_duplicated_auth_key_as_revoked() {
        assert!(is_auth_revoked_message(
            "telegram error: RPC 406: AUTH_KEY_DUPLICATED"
        ));
    }

    #[test]
    fn detects_revoked_auth_markers() {
        assert!(is_auth_revoked_message(
            "telegram error: AUTH_KEY_UNREGISTERED"
        ));
        assert!(is_auth_revoked_message(
            "RpcError { name: SESSION_REVOKED }"
        ));
        assert!(is_auth_revoked_message("user_deactivated_ban"));
        assert!(!is_auth_revoked_message("telegram unreachable"));
        assert!(!is_auth_revoked_message("FLOOD_WAIT_30"));
    }
}
