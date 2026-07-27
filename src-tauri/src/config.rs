//! Telegram API credentials.
//!
//! `api_id`/`api_hash` are, as grammers recommends, embedded into the binary at
//! build time via `TELEGRAM_API_ID` / `TELEGRAM_API_HASH`. `build.rs` loads
//! gitignored `src-tauri/.env.local` when those vars are unset (local builds);
//! CI should export them as secrets. Runtime env still overrides the embed
//! (handy for `tauri dev` without a rebuild).

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct Config {
    pub api_id: i32,
    pub api_hash: String,
}

impl Config {
    /// Resolves credentials from the runtime environment first (dev override),
    /// then the values embedded at compile time.
    pub fn load() -> AppResult<Self> {
        #[cfg(debug_assertions)]
        {
            // Optional: don't fail if file is missing
            let _ = dotenvy::from_filename(".env.local");
        }

        let api_id_raw = std::env::var("TELEGRAM_API_ID")
            .ok()
            .or_else(|| option_env!("TELEGRAM_API_ID").map(str::to_owned))
            .ok_or_else(|| {
                AppError::msg(
                    "TELEGRAM_API_ID is not set. Provide it at build time or via the environment.",
                )
            })?;

        let api_hash = std::env::var("TELEGRAM_API_HASH")
            .ok()
            .or_else(|| option_env!("TELEGRAM_API_HASH").map(str::to_owned))
            .ok_or_else(|| {
                AppError::msg(
                    "TELEGRAM_API_HASH is not set. Provide it at build time or via the environment.",
                )
            })?;

        let api_id = api_id_raw
            .trim()
            .parse::<i32>()
            .map_err(|_| AppError::msg("TELEGRAM_API_ID must be an integer"))?;

        Ok(Self { api_id, api_hash })
    }
}
