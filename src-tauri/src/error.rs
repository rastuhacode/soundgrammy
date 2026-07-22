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

    #[error("telegram error: {0}")]
    Telegram(String),

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
    /// Builds a generic, user-facing error from any displayable value.
    pub fn msg(message: impl Into<String>) -> Self {
        AppError::Message(message.into())
    }
}

impl From<ferogram::InvocationError> for AppError {
    fn from(value: ferogram::InvocationError) -> Self {
        AppError::Telegram(value.to_string())
    }
}

/// Serialize as `{ "message": "..." }` so the frontend can read `err.message`.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 1)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

/// Convenience result alias for command bodies.
pub type AppResult<T> = Result<T, AppError>;
