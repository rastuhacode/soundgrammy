//! Authentication flows: phone + 2FA and QR login via ferogram.

use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use ferogram::tl;
use ferogram::{Client, InvocationError, PasswordToken, SendCodeOutcome, SignInError};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, AppResult};
use crate::state::{AppState, PendingAuth};
use crate::telegram::client;

/// How long background online auth refresh may wait before treating Telegram as unreachable.
const REFRESH_AUTH_TIMEOUT: Duration = Duration::from_secs(8);

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
async fn finalize(state: &AppState, client: &Client, epoch: u64) -> AppResult<AuthUser> {
    let user = fetch_self(client).await?;
    let _guard = state.guard_client(epoch).await?;
    client.save_session().await?;
    state.db.save_profile(
        user.id,
        &user.first_name,
        user.last_name.as_deref(),
        user.username.as_deref(),
        user.phone.as_deref(),
    )?;
    *state.pending.lock().await = PendingAuth::Idle;
    Ok(user)
}

fn clear_local_session(state: &AppState) -> AppResult<()> {
    state.audio.clear_session();
    if let Ok(Some(profile)) = state.db.load_profile() {
        state.db.clear_active_profile(profile.tg_user_id)?;
    }
    crate::session::clear(&state.data_dir)?;
    Ok(())
}

async fn clear_local_session_and_pending(state: &AppState) -> AppResult<()> {
    // Explicit logout/restart applies to the current client, including a rebuild
    // that completed while this operation waited for the lifecycle lock.
    let _guard = state.guard_lifecycle().await;
    state.disconnect_client_locked().await;
    clear_local_session(state)
}
async fn clear_local_session_at(state: &AppState, epoch: u64) -> AppResult<()> {
    let _guard = state.guard_client(epoch).await?;
    state.disconnect_client_locked().await;
    clear_local_session(state)
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

    let generation = state.db.account_generation()?;
    let expected_uid = state
        .db
        .load_profile()?
        .ok_or(AppError::NotAuthorized)?
        .tg_user_id;
    let client_epoch = std::sync::atomic::AtomicU64::new(
        state
            .client_generation
            .load(std::sync::atomic::Ordering::Acquire),
    );
    let result = tokio::time::timeout(REFRESH_AUTH_TIMEOUT, async {
        state.ensure_client().await?;
        let (client, epoch) = state.auth_client().await?;
        client_epoch.store(epoch, std::sync::atomic::Ordering::Release);
        let authorized = client::is_authorized(&client).await?;
        if !authorized {
            return Ok::<AuthStatus, AppError>(AuthStatus {
                authorized: false,
                user: None,
            });
        }
        let user = fetch_self(&client).await?;
        if user.id != expected_uid {
            return Err(AppError::NotAuthorized);
        }
        state.db.refresh_profile_at_generation(
            user.id,
            generation,
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

    if state.db.account_generation()? != generation {
        return Err(AppError::NotAuthorized);
    }
    match result {
        Ok(Ok(status)) => {
            if !status.authorized {
                // Session file present but client reports unauthorized — treat as revoked.
                clear_local_session_at(
                    state,
                    client_epoch.load(std::sync::atomic::Ordering::Acquire),
                )
                .await?;
                emit_auth_revoked(app);
            }
            Ok(status)
        }
        Ok(Err(err)) => {
            if err.is_auth_revoked() {
                clear_local_session_at(
                    state,
                    client_epoch.load(std::sync::atomic::Ordering::Acquire),
                )
                .await?;
                emit_auth_revoked(app);
                return Ok(AuthStatus {
                    authorized: false,
                    user: None,
                });
            }
            // Unreachable / transient — keep local session; surface as soft error for UI.
            if let Ok(_guard) = state
                .guard_client(client_epoch.load(std::sync::atomic::Ordering::Acquire))
                .await
            {
                state.disconnect_client_locked().await;
            }
            Err(AppError::msg("telegram unreachable"))
        }
        Err(_elapsed) => {
            if let Ok(_guard) = state
                .guard_client(client_epoch.load(std::sync::atomic::Ordering::Acquire))
                .await
            {
                state.disconnect_client_locked().await;
            }
            Err(AppError::msg("telegram unreachable"))
        }
    }
}

// ---- phone and QR flows --------------------------------------------------

const AUTH_STEP_TIMEOUT: Duration = Duration::from_secs(15);

pub async fn phone_send_code(state: &AppState, phone: &str) -> AppResult<PhoneSendCodeOutcome> {
    let _operation = state.auth_operations.lock().await;
    let (client, epoch) = state.auth_client().await?;
    *state.pending.lock().await = PendingAuth::Idle;
    let outcome = tokio::time::timeout(AUTH_STEP_TIMEOUT, client.request_login_code(phone))
        .await
        .map_err(|_| AppError::msg("Telegram login timed out; retry"))??;
    match outcome {
        SendCodeOutcome::CodeRequired(token) => {
            let _guard = state.guard_client(epoch).await?;
            *state.pending.lock().await = PendingAuth::Phone(token);
            Ok(PhoneSendCodeOutcome::CodeSent)
        }
        SendCodeOutcome::AlreadyAuthorized(_) => Ok(PhoneSendCodeOutcome::Authorized {
            user: finalize(state, &client, epoch).await?,
        }),
    }
}

pub async fn phone_sign_in(state: &AppState, code: &str) -> AppResult<AuthOutcome> {
    let _operation = state.auth_operations.lock().await;
    let (client, epoch) = state.auth_client().await?;
    let token = match &*state.pending.lock().await {
        PendingAuth::Phone(token) => token.clone(),
        _ => return Err(AppError::msg("no login in progress; request a code first")),
    };
    let result = tokio::time::timeout(AUTH_STEP_TIMEOUT, client.sign_in(&token, code))
        .await
        .map_err(|_| AppError::msg("Telegram login timed out; retry"))?;
    match result {
        Ok(_) => Ok(AuthOutcome::Authorized {
            user: finalize(state, &client, epoch).await?,
        }),
        Err(SignInError::PasswordRequired(token)) => {
            let _guard = state.guard_client(epoch).await?;
            let hint = token.hint().map(str::to_owned);
            *state.pending.lock().await = PendingAuth::Password(token);
            Ok(AuthOutcome::PasswordRequired { hint })
        }
        Err(SignInError::InvalidCode) => Err(AppError::msg("the code you entered is invalid")),
        Err(error) => Err(InvocationError::from(error).into()),
    }
}

pub async fn check_password(state: &AppState, password: &str) -> AppResult<AuthUser> {
    let _operation = state.auth_operations.lock().await;
    let (client, epoch) = state.auth_client().await?;
    let token = match &*state.pending.lock().await {
        PendingAuth::Password(token) => (**token).clone(),
        _ => return Err(AppError::msg("no password step in progress")),
    };
    let result = tokio::time::timeout(
        AUTH_STEP_TIMEOUT,
        client.check_password(token, password.as_bytes()),
    )
    .await
    .map_err(|_| AppError::msg("Telegram login timed out; retry"))?;
    match result {
        Ok(_) => finalize(state, &client, epoch).await,
        Err(InvocationError::Rpc(error)) if error.name == "PASSWORD_HASH_INVALID" => {
            if let Ok(token) = load_password_token(&client).await {
                let _guard = state.guard_client(epoch).await?;
                *state.pending.lock().await = PendingAuth::Password(Box::new(token));
            }
            Err(AppError::msg("incorrect password"))
        }
        Err(error) => Err(error.into()),
    }
}

async fn qr_export_once(state: &AppState, client: &Client, epoch: u64) -> AppResult<QrOutcome> {
    let result = tokio::time::timeout(AUTH_STEP_TIMEOUT, client.export_login_token())
        .await
        .map_err(|_| AppError::msg("Telegram login timed out; retry"))?;
    match result {
        Ok((token, _)) if token.is_empty() => Ok(QrOutcome::Authorized {
            user: finalize(state, client, epoch).await?,
        }),
        Ok((token, expires)) => {
            let _guard = state.guard_client(epoch).await?;
            *state.pending.lock().await = PendingAuth::Qr;
            Ok(QrOutcome::Waiting {
                url: format!("tg://login?token={}", B64URL.encode(token)),
                expires,
            })
        }
        Err(InvocationError::Rpc(error)) if error.name == "SESSION_PASSWORD_NEEDED" => {
            let token = load_password_token(client).await?;
            let hint = token.hint().map(str::to_owned);
            let _guard = state.guard_client(epoch).await?;
            *state.pending.lock().await = PendingAuth::Password(Box::new(token));
            Ok(QrOutcome::PasswordRequired { hint })
        }
        Err(error) => Err(error.into()),
    }
}

async fn qr_export_locked(state: &AppState) -> AppResult<QrOutcome> {
    let (client, epoch) = state.auth_client().await?;
    match qr_export_once(state, &client, epoch).await {
        Err(error) if error.is_auth_revoked() => {
            clear_local_session_at(state, epoch).await?;
            state.ensure_client().await?;
            let (client, epoch) = state.auth_client().await?;
            qr_export_once(state, &client, epoch).await
        }
        result => result,
    }
}
pub async fn qr_export(state: &AppState) -> AppResult<QrOutcome> {
    let _operation = state.auth_operations.lock().await;
    qr_export_locked(state).await
}
pub async fn qr_restart(state: &AppState) -> AppResult<QrOutcome> {
    let _operation = state.auth_operations.lock().await;
    clear_local_session_and_pending(state).await?;
    state.ensure_client().await?;
    qr_export_locked(state).await
}
pub async fn qr_check_password(state: &AppState, password: &str) -> AppResult<AuthUser> {
    check_password(state, password).await
}
async fn load_password_token(client: &Client) -> AppResult<PasswordToken> {
    let password = tokio::time::timeout(
        AUTH_STEP_TIMEOUT,
        client.invoke(&tl::functions::account::GetPassword {}),
    )
    .await
    .map_err(|_| AppError::msg("Telegram password challenge timed out; retry"))??;
    let tl::enums::account::Password::Password(password) = password;
    Ok(PasswordToken { password })
}

// ---- session lifecycle ---------------------------------------------------

pub async fn logout(state: &AppState) -> AppResult<()> {
    let _operation = state.auth_operations.lock().await;
    let remote = state.auth_client().await.ok().map(|(client, _)| client);
    // Clear locally first; remote availability cannot keep the native account alive.
    clear_local_session_and_pending(state).await?;
    if let Some(client) = remote {
        let _ = tokio::time::timeout(Duration::from_secs(3), client.sign_out()).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn offline_code_submission_keeps_its_retry_token_and_logout_invalidates_it() {
        let state = crate::cache::tests::test_state();
        state.db.save_profile(1, "User", None, None, None).unwrap();
        *state.pending.lock().await = PendingAuth::Phone(ferogram::LoginToken {
            phone: "test".into(),
            phone_code_hash: "test-hash".into(),
        });
        let generation = state.db.account_generation().unwrap();
        assert!(phone_sign_in(&state, "123").await.is_err());
        assert!(
            matches!(&*state.pending.lock().await,PendingAuth::Phone(token) if token.phone_code_hash=="test-hash")
        );
        let old_client = state
            .client_generation
            .load(std::sync::atomic::Ordering::Acquire);
        logout(&state).await.unwrap();
        assert!(matches!(*state.pending.lock().await, PendingAuth::Idle));
        assert!(state.guard_client(old_client).await.is_err());
        assert!(state
            .db
            .refresh_profile_at_generation(1, generation, "Late", None, None, None)
            .is_err());
        assert!(state.db.load_profile().unwrap().is_none());
    }
    #[tokio::test]
    async fn explicit_logout_survives_a_reconnect_that_finishes_while_it_waits() {
        let state = std::sync::Arc::new(crate::cache::tests::test_state());
        state.db.save_profile(1, "User", None, None, None).unwrap();
        let guard = state.guard_lifecycle().await;
        let task = {
            let state = state.clone();
            tokio::spawn(async move { clear_local_session_and_pending(&state).await })
        };
        tokio::task::yield_now().await;
        state
            .client_generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        drop(guard);
        task.await.unwrap().unwrap();
        assert!(state.db.load_profile().unwrap().is_none());
    }
}
