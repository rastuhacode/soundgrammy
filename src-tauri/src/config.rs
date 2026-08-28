//! Backend-only Telegram and optional Last.fm application credentials.
//!
//! `api_id`/`api_hash` are embedded into the binary at build time via
//! `TELEGRAM_API_ID` / `TELEGRAM_API_HASH`. `build.rs` loads
//! gitignored `src-tauri/.env.local` when those vars are unset (local builds);
//! CI should export them as secrets. Runtime env still overrides the embed
//! (handy for `tauri dev` without a rebuild).

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct Config {
    pub api_id: i32,
    pub api_hash: String,
    pub lastfm: Option<LastFmCredentials>,
}

#[derive(Clone)]
pub struct LastFmCredentials {
    pub api_key: String,
    pub api_secret: String,
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

        let lastfm_key = non_blank_env("LASTFM_API_KEY")
            .or_else(|| option_env!("LASTFM_API_KEY").and_then(non_blank));
        let lastfm_secret = non_blank_env("LASTFM_API_SECRET")
            .or_else(|| option_env!("LASTFM_API_SECRET").and_then(non_blank));
        let lastfm = match (lastfm_key, lastfm_secret) {
            (Some(api_key), Some(api_secret)) => Some(LastFmCredentials {
                api_key,
                api_secret,
            }),
            _ => None,
        };

        Ok(Self {
            api_id,
            api_hash,
            lastfm,
        })
    }
}

fn non_blank_env(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|value| non_blank(&value))
}

fn non_blank(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
