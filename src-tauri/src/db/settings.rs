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
pub const SETTING_LASTFM_USERNAME: &str = "lastfm_username";
pub const SETTING_LASTFM_ACCOUNT_KEY: &str = "lastfm_account_key";
pub const SETTING_LASTFM_ENABLED: &str = "lastfm_enabled";
pub const SETTING_LASTFM_LAST_SCROBBLE_AT_MS: &str = "lastfm_last_scrobble_at_ms";
pub const SETTING_LASTFM_NEEDS_REAUTH: &str = "lastfm_needs_reauth";

impl Db {
    /// Read related settings from one connection snapshot.
    pub fn settings_snapshot(
        &self,
        keys: &[&str],
    ) -> AppResult<std::collections::HashMap<String, String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key=?1")?;
        let mut result = std::collections::HashMap::new();
        for key in keys {
            if let Some(value) = stmt
                .query_row([key], |row| row.get::<_, String>(0))
                .optional()?
            {
                result.insert((*key).into(), value);
            }
        }
        Ok(result)
    }

    /// Persist a configuration as a unit; None removes a setting.
    pub fn update_settings(&self, changes: &[(&str, Option<&str>)]) -> AppResult<()> {
        self.update_settings_and_queue(changes, None)
    }
    pub fn update_settings_and_queue(
        &self,
        changes: &[(&str, Option<&str>)],
        delete_account: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        for (key, value) in changes {
            match value {
                Some(value) => {
                    tx.execute(
                        "INSERT INTO app_settings(key,value) VALUES(?1,?2) \
                    ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                        params![key, value],
                    )?;
                }
                None => {
                    tx.execute("DELETE FROM app_settings WHERE key=?1", [key])?;
                }
            }
        }
        if let Some(account) = delete_account {
            tx.execute(
                "DELETE FROM lastfm_scrobble_queue WHERE lastfm_account_key=?1",
                [account],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

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
