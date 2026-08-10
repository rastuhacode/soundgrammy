use rusqlite::{params, OptionalExtension};

use super::Db;
use crate::error::AppResult;

/// Default audio cache size limit: 5 GiB.
pub const DEFAULT_CACHE_LIMIT_BYTES: i64 = 5_368_709_120;
/// Default cache TTL: 30 days.
pub const DEFAULT_CACHE_TTL_SECS: i64 = 2_592_000;

pub const SETTING_CACHE_LIMIT_BYTES: &str = "cache_limit_bytes";
pub const SETTING_CACHE_TTL_SECS: &str = "cache_ttl_secs";
pub const SETTING_LISTEN_STATS_ENABLED: &str = "listen_stats_enabled";

impl Db {
    pub fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let value = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting_i64(&self, key: &str, default: i64) -> AppResult<i64> {
        match self.get_setting(key)? {
            Some(raw) => Ok(raw.parse::<i64>().unwrap_or(default)),
            None => Ok(default),
        }
    }
}
