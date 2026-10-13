//! A single application error type shared by every Tauri command.
//!
//! All fallible boundaries collapse into [`AppError`]. It implements
//! [`serde::Serialize`] so a rejected command surfaces on the frontend as a
//! plain `{ message }` object, mirroring how the old tRPC layer reported errors.

use serde::{Serialize, Serializer};

/// The error type returned by every command in the app.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not authorized: sign in first")]
    NotAuthorized,

    #[error("Credential recovery failed; reconnect Last.fm to restore a consistent account")]
    CredentialRecovery,

    #[error("telegram error: {0}")]
    Telegram(#[from] ferogram::InvocationError),

    #[error("{0}")]
    Message(String),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl AppError {
    pub fn is_auth_revoked(&self) -> bool {
        matches!(self,Self::Telegram(ferogram::InvocationError::Rpc(rpc)) if matches!(rpc.name.as_str(),
            "AUTH_KEY_DUPLICATED"|"AUTH_KEY_UNREGISTERED"|"AUTH_KEY_INVALID"|"SESSION_REVOKED"|
            "SESSION_EXPIRED"|"USER_DEACTIVATED"|"USER_DEACTIVATED_BAN"))
    }
    /// Builds a generic, user-facing error from any displayable value.
    pub fn msg(message: impl Into<String>) -> Self {
        AppError::Message(message.into())
    }
}

/// Serialize as `{ "message": "..." }` so the frontend can read `err.message`.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 4)?;
        state.serialize_field("message", &self.to_string())?;
        let (code, retryable, retry_after_seconds) = match self {
            Self::CredentialRecovery => ("credential_recovery", false, None),
            Self::NotAuthorized => ("not_authorized", false, None),
            Self::Telegram(ferogram::InvocationError::Rpc(rpc)) if rpc.code == 420 => {
                ("telegram_flood_wait", true, rpc.value)
            }
            Self::Telegram(ferogram::InvocationError::Rpc(rpc)) => {
                ("telegram_rpc", rpc.code >= 500, None)
            }
            Self::Telegram(_) => ("telegram_transport", true, None),
            Self::Db(_) => ("database", false, None),
            Self::Io(_) => ("io", false, None),
            Self::Serde(_) => ("invalid_payload", false, None),
            Self::Message(_) => ("application", false, None),
        };
        state.serialize_field("code", code)?;
        state.serialize_field("retryable", &retryable)?;
        state.serialize_field("retryAfterSeconds", &retry_after_seconds)?;
        state.end()
    }
}

/// Convenience result alias for command bodies.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rpc_identity_and_retry_deadline_survive_the_application_boundary() {
        let error = AppError::from(ferogram::InvocationError::Rpc(ferogram::RpcError {
            code: 420,
            name: "FLOOD_WAIT".into(),
            value: Some(37),
        }));
        let value = serde_json::to_value(error).unwrap();
        assert_eq!(value["code"], "telegram_flood_wait");
        assert_eq!(value["retryAfterSeconds"], 37);
        assert_eq!(value["retryable"], true);
        assert!(!AppError::msg("text mentioning AUTH_KEY_INVALID").is_auth_revoked());
        assert!(
            AppError::from(ferogram::InvocationError::Rpc(ferogram::RpcError {
                code: 401,
                name: "AUTH_KEY_INVALID".into(),
                value: None
            }))
            .is_auth_revoked()
        );
    }
}
