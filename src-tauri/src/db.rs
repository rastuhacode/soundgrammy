//! Local SQLite library (tracks + playlists), recreated from scratch.
//!
//! Ported from the web app's `lib/db.ts`, minus the session/pending-auth tables
//! (those are replaced by the encrypted session store). A single connection is
//! guarded by a `Mutex`; persistence is grouped into focused domain modules.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

mod bounce_profiles;
mod cache_entries;
mod lastfm;
mod listen_stats;
mod models;
mod playlists;
mod profile;
mod schema;
mod settings;
#[cfg(test)]
mod tests;
mod tracks;

pub use cache_entries::*;
pub use lastfm::*;
pub use models::*;
pub use settings::*;

use crate::error::AppResult;

pub struct Db {
    pub(super) conn: Mutex<Connection>,
}

impl Db {
    /// Opens (creating if needed) the database at `path` and applies the schema.
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::apply(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}
