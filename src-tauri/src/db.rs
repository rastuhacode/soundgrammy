//! Local SQLite library (tracks + playlists), recreated from scratch.
//!
//! Ported from the web app's `lib/db.ts`, minus the session/pending-auth tables
//! (those are replaced by the encrypted session store). A single connection is
//! guarded by a `Mutex`; persistence is grouped into focused domain modules.

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

mod bounce_profiles;
mod cache_entries;
mod lastfm;
mod library;
mod listen_stats;
mod models;
mod playlists;
mod profile;
mod schema;
mod settings;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::test_db;
mod tracks;

pub use cache_entries::*;
pub use lastfm::*;
pub use models::*;
pub use settings::*;

use crate::error::AppResult;

#[derive(Clone)]
pub struct Db {
    pub(super) conn: Arc<Mutex<Connection>>,
}

impl Db {
    /// Keep synchronous SQLite/file work off the async executor.
    pub async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&Db) -> AppResult<T> + Send + 'static,
    ) -> AppResult<T> {
        let db = self.clone();
        tokio::task::spawn_blocking(move || operation(&db))
            .await
            .map_err(|_| crate::error::AppError::msg("Database worker interrupted"))?
    }
    pub fn active_uid(&self) -> AppResult<i64> {
        self.load_profile()?
            .map(|p| p.tg_user_id)
            .ok_or(crate::error::AppError::NotAuthorized)
    }
    /// Opens (creating if needed) the database at `path` and applies the schema.
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::apply(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}
