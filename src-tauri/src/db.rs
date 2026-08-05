//! Local SQLite library (tracks + playlists), recreated from scratch.
//!
//! Ported from the web app's `lib/db.ts`, minus the session/pending-auth tables
//! (those are replaced by the encrypted session store). A single connection is
//! guarded by a `Mutex`; all access goes through small prepared-statement
//! helpers — no ORM.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::AppResult;
use crate::listen_stats::{
    apply_attempt_end, compute_likeness, derive_attempt_end, AttemptEndInput, EndReason,
    ListenAggregates,
};

pub struct Db {
    conn: Mutex<Connection>,
}

/// A track as sent to the frontend. Field names are snake_case to match the
/// ported Zustand stores/components.
#[derive(Debug, Clone, Serialize)]
pub struct Track {
    pub id: i64,
    pub tg_user_id: i64,
    pub file_id: String,
    pub file_unique_id: String,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub duration: Option<i64>,
    pub source: String,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub created_at: String,
    /// The serialized Telegram document JSON stays server-side only.
    #[serde(skip)]
    pub mtproto_document: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LikedPlaylist {
    pub id: i64,
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<i64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomPlaylistSummary {
    pub id: i64,
    pub name: String,
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<i64>,
    #[serde(rename = "hasThumbnail")]
    pub has_thumbnail: bool,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistsBundle {
    pub liked: LikedPlaylist,
    pub custom: Vec<CustomPlaylistSummary>,
}

/// Fields needed to upsert a track discovered during saved-music sync.
pub struct UpsertTrack {
    pub tg_user_id: i64,
    pub file_id: String,
    pub file_unique_id: String,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub duration: Option<i64>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub track_position: i64,
    pub mtproto_document: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackListenStats {
    pub track_id: i64,
    pub starts: i64,
    pub qualified_plays: i64,
    pub completes: i64,
    pub early_skips: i64,
    pub total_listened_ms: i64,
    pub first_played_at_ms: Option<i64>,
    pub last_played_at_ms: Option<i64>,
    pub likeness: f64,
}

#[derive(Debug, Clone)]
pub struct TrackBounceProfileRecord {
    pub track_id: i64,
    pub algorithm_version: i64,
    pub frame_ms: i64,
    pub duration_ms: i64,
    pub file_size: Option<i64>,
    pub loudness: Vec<u8>,
    pub onset: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListenEndResult {
    pub qualified: bool,
    pub early_skip: bool,
    pub listened_eff_ms: i64,
    pub stats: TrackListenStats,
}

impl Db {
    /// Opens (creating if needed) the database at `path` and applies the schema.
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Self::apply_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn apply_schema(conn: &Connection) -> AppResult<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tracks (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              tg_user_id INTEGER NOT NULL,
              file_id TEXT NOT NULL,
              file_unique_id TEXT NOT NULL UNIQUE,
              title TEXT,
              performer TEXT,
              duration INTEGER,
              source TEXT NOT NULL DEFAULT 'mtproto',
              mime_type TEXT,
              file_size INTEGER,
              mtproto_document TEXT,
              track_position INTEGER,
              created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_tracks_tg_user_id ON tracks (tg_user_id);

            CREATE TABLE IF NOT EXISTS playlists (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              tg_user_id INTEGER NOT NULL,
              name TEXT NOT NULL,
              kind TEXT NOT NULL CHECK (kind IN ('liked', 'custom')),
              thumbnail_data TEXT,
              thumbnail_mime TEXT,
              created_at TEXT NOT NULL DEFAULT (datetime('now')),
              updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_playlists_user_liked
              ON playlists (tg_user_id) WHERE kind = 'liked';

            CREATE TABLE IF NOT EXISTS playlist_tracks (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
              track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
              position INTEGER NOT NULL DEFAULT 0,
              added_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist_position
              ON playlist_tracks (playlist_id, position);

            CREATE TABLE IF NOT EXISTS profile (
              tg_user_id INTEGER PRIMARY KEY,
              first_name TEXT NOT NULL,
              last_name TEXT,
              username TEXT,
              phone TEXT
            );

            CREATE TABLE IF NOT EXISTS app_meta (
              tg_user_id INTEGER PRIMARY KEY,
              saved_music_hash TEXT,
              last_sync_at TEXT
            );

            CREATE TABLE IF NOT EXISTS app_settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS listen_events (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              track_id INTEGER NOT NULL,
              event_type TEXT NOT NULL CHECK (event_type IN ('attempt_start', 'attempt_end')),
              ts_ms INTEGER NOT NULL,
              listened_ms INTEGER,
              duration_ms INTEGER,
              end_reason TEXT,
              qualified INTEGER NOT NULL DEFAULT 0,
              early_skip INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_listen_events_track_id
              ON listen_events (track_id);

            CREATE TABLE IF NOT EXISTS track_listen_stats (
              track_id INTEGER PRIMARY KEY,
              starts INTEGER NOT NULL DEFAULT 0,
              qualified_plays INTEGER NOT NULL DEFAULT 0,
              completes INTEGER NOT NULL DEFAULT 0,
              early_skips INTEGER NOT NULL DEFAULT 0,
              total_listened_ms INTEGER NOT NULL DEFAULT 0,
              first_played_at_ms INTEGER,
              last_played_at_ms INTEGER,
              likeness REAL NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS track_bounce_profiles (
              track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
              algorithm_version INTEGER NOT NULL,
              frame_ms INTEGER NOT NULL,
              duration_ms INTEGER NOT NULL,
              file_size INTEGER,
              loudness BLOB NOT NULL,
              onset BLOB NOT NULL,
              created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            "#,
        )?;
        Self::migrate_playlists_updated_at(conn)?;
        Self::migrate_playlist_tracks_entry_ids(conn)?;
        Ok(())
    }

    fn migrate_playlists_updated_at(conn: &Connection) -> AppResult<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(playlists)")?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if columns.iter().any(|c| c == "updated_at") {
            return Ok(());
        }
        conn.execute_batch(
            "ALTER TABLE playlists ADD COLUMN updated_at TEXT NOT NULL DEFAULT (datetime('now'));
             UPDATE playlists SET updated_at = created_at;",
        )?;
        Ok(())
    }

    /// Allow duplicate track entries in a playlist (ordered slots with a row id).
    fn migrate_playlist_tracks_entry_ids(conn: &Connection) -> AppResult<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(playlist_tracks)")?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if columns.iter().any(|c| c == "id") {
            return Ok(());
        }
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             CREATE TABLE playlist_tracks_new (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
               track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
               position INTEGER NOT NULL DEFAULT 0,
               added_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             INSERT INTO playlist_tracks_new (playlist_id, track_id, position, added_at)
               SELECT playlist_id, track_id, position, added_at FROM playlist_tracks
               ORDER BY playlist_id ASC, position ASC, added_at ASC;
             DROP TABLE playlist_tracks;
             ALTER TABLE playlist_tracks_new RENAME TO playlist_tracks;
             CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist_position
               ON playlist_tracks (playlist_id, position);
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(())
    }

    fn playlist_updated_at(conn: &Connection, playlist_id: i64) -> AppResult<String> {
        Ok(conn.query_row(
            "SELECT updated_at FROM playlists WHERE id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?)
    }

    fn touch_playlist_updated_at(conn: &Connection, playlist_id: i64) -> AppResult<String> {
        conn.execute(
            "UPDATE playlists SET updated_at = datetime('now') WHERE id = ?1",
            params![playlist_id],
        )?;
        Self::playlist_updated_at(conn, playlist_id)
    }

    // ---- tracks ---------------------------------------------------------

    pub fn tracks_by_user(&self, tg_user_id: i64) -> AppResult<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tg_user_id, file_id, file_unique_id, title, performer, duration, \
             source, mime_type, file_size, created_at, mtproto_document \
             FROM tracks WHERE tg_user_id = ?1 \
             ORDER BY track_position ASC, created_at DESC, id DESC",
        )?;
        let rows = stmt
            .query_map(params![tg_user_id], map_track)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn track_by_id(&self, id: i64, tg_user_id: i64) -> AppResult<Option<Track>> {
        let conn = self.conn.lock().unwrap();
        let track = conn
            .query_row(
                "SELECT id, tg_user_id, file_id, file_unique_id, title, performer, duration, \
                 source, mime_type, file_size, created_at, mtproto_document \
                 FROM tracks WHERE id = ?1 AND tg_user_id = ?2",
                params![id, tg_user_id],
                map_track,
            )
            .optional()?;
        Ok(track)
    }

    /// Looks up local track ids by Telegram document id (`file_unique_id`).
    pub fn track_ids_by_file_unique_ids(
        &self,
        tg_user_id: i64,
        file_unique_ids: &[String],
    ) -> AppResult<std::collections::HashMap<String, i64>> {
        if file_unique_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let conn = self.conn.lock().unwrap();
        let mut map = std::collections::HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT id, file_unique_id FROM tracks \
             WHERE tg_user_id = ?1 AND file_unique_id = ?2",
        )?;
        for uid in file_unique_ids {
            if map.contains_key(uid) {
                continue;
            }
            if let Some((id, unique)) = stmt
                .query_row(params![tg_user_id, uid], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .optional()?
            {
                map.insert(unique, id);
            }
        }
        Ok(map)
    }

    /// Ordered track rows for a playlist (duplicates preserved as separate positions).
    pub fn playlist_tracks_ordered(
        &self,
        playlist_id: i64,
        tg_user_id: i64,
    ) -> AppResult<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
                params![playlist_id, tg_user_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(crate::error::AppError::msg("Playlist not found"));
        }
        let mut stmt = conn.prepare(
            "SELECT t.id, t.tg_user_id, t.file_id, t.file_unique_id, t.title, t.performer, \
             t.duration, t.source, t.mime_type, t.file_size, t.created_at, t.mtproto_document \
             FROM playlist_tracks pt \
             JOIN tracks t ON t.id = pt.track_id \
             WHERE pt.playlist_id = ?1 \
             ORDER BY pt.position ASC, pt.added_at ASC, pt.id ASC",
        )?;
        let tracks = stmt
            .query_map(params![playlist_id], map_track)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tracks)
    }

    pub fn liked_playlist_id(&self, tg_user_id: i64) -> AppResult<i64> {
        let conn = self.conn.lock().unwrap();
        self.ensure_liked(&conn, tg_user_id)
    }

    pub fn custom_playlist_name_and_kind(
        &self,
        playlist_id: i64,
        tg_user_id: i64,
    ) -> AppResult<(String, String)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT name, kind FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
            params![playlist_id, tg_user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| crate::error::AppError::msg("Playlist not found"))
    }

    pub fn upsert_track(&self, t: &UpsertTrack) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tracks (tg_user_id, file_id, file_unique_id, title, performer, duration, \
             source, mime_type, file_size, track_position, mtproto_document) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'mtproto', ?7, ?8, ?9, ?10) \
             ON CONFLICT (file_unique_id) DO UPDATE SET \
               file_id = excluded.file_id, title = excluded.title, performer = excluded.performer, \
               duration = excluded.duration, mime_type = excluded.mime_type, \
               file_size = excluded.file_size, track_position = excluded.track_position, \
               mtproto_document = excluded.mtproto_document",
            params![
                t.tg_user_id,
                t.file_id,
                t.file_unique_id,
                t.title,
                t.performer,
                t.duration,
                t.mime_type,
                t.file_size,
                t.track_position,
                t.mtproto_document,
            ],
        )?;
        Ok(())
    }

    /// Updates the stored document JSON after a file-reference refresh.
    pub fn update_track_document(&self, id: i64, tg_user_id: i64, doc_json: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tracks SET mtproto_document = ?1 WHERE id = ?2 AND tg_user_id = ?3",
            params![doc_json, id, tg_user_id],
        )?;
        Ok(())
    }

    /// Corrects a track MIME after on-disk content sniffing disagrees with Telegram.
    pub fn update_track_mime(
        &self,
        id: i64,
        tg_user_id: i64,
        mime_type: &str,
    ) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tracks SET mime_type = ?1 WHERE id = ?2 AND tg_user_id = ?3",
            params![mime_type, id, tg_user_id],
        )?;
        Ok(())
    }

    /// Removes mtproto tracks whose `file_unique_id` isn't in `keep`, returning
    /// the number of removed rows.
    pub fn delete_tracks_not_in(&self, tg_user_id: i64, keep: &[String]) -> AppResult<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let removed = if keep.is_empty() {
            tx.execute(
                "DELETE FROM tracks WHERE tg_user_id = ?1 AND source = 'mtproto'",
                params![tg_user_id],
            )?
        } else {
            let placeholders = std::iter::repeat("?")
                .take(keep.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM tracks WHERE tg_user_id = ?1 AND source = 'mtproto' \
                 AND file_unique_id NOT IN ({placeholders})"
            );
            let mut bind: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(keep.len() + 1);
            bind.push(&tg_user_id);
            for k in keep {
                bind.push(k);
            }
            tx.execute(&sql, bind.as_slice())?
        };
        tx.commit()?;
        Ok(removed)
    }

    // ---- profile & sync meta -------------------------------------------

    pub fn save_profile(
        &self,
        tg_user_id: i64,
        first_name: &str,
        last_name: Option<&str>,
        username: Option<&str>,
        phone: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO profile (tg_user_id, first_name, last_name, username, phone) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT (tg_user_id) DO UPDATE SET \
               first_name = excluded.first_name, last_name = excluded.last_name, \
               username = excluded.username, phone = excluded.phone",
            params![tg_user_id, first_name, last_name, username, phone],
        )?;
        Ok(())
    }

    pub fn load_profile(&self) -> AppResult<Option<Profile>> {
        let conn = self.conn.lock().unwrap();
        let profile = conn
            .query_row(
                "SELECT tg_user_id, first_name, last_name, username, phone FROM profile LIMIT 1",
                [],
                |row| {
                    Ok(Profile {
                        tg_user_id: row.get(0)?,
                        first_name: row.get(1)?,
                        last_name: row.get(2)?,
                        username: row.get(3)?,
                        phone: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(profile)
    }

    pub fn clear_active_profile(&self, tg_user_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM profile WHERE tg_user_id = ?1",
            params![tg_user_id],
        )?;
        Ok(())
    }

    pub fn saved_music_hash(&self, tg_user_id: i64) -> AppResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let hash = conn
            .query_row(
                "SELECT saved_music_hash FROM app_meta WHERE tg_user_id = ?1",
                params![tg_user_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(hash)
    }

    pub fn last_sync_at(&self, tg_user_id: i64) -> AppResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let value = conn
            .query_row(
                "SELECT last_sync_at FROM app_meta WHERE tg_user_id = ?1",
                params![tg_user_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(value)
    }

    pub fn set_saved_music_hash(&self, tg_user_id: i64, hash: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_meta (tg_user_id, saved_music_hash) VALUES (?1, ?2) \
             ON CONFLICT (tg_user_id) DO UPDATE SET saved_music_hash = excluded.saved_music_hash",
            params![tg_user_id, hash],
        )?;
        Ok(())
    }

    pub fn mark_synced(&self, tg_user_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_meta (tg_user_id, last_sync_at) VALUES (?1, datetime('now')) \
             ON CONFLICT (tg_user_id) DO UPDATE SET last_sync_at = datetime('now')",
            params![tg_user_id],
        )?;
        Ok(())
    }

    // ---- playlists ------------------------------------------------------

    fn ensure_liked(&self, conn: &Connection, tg_user_id: i64) -> AppResult<i64> {
        if let Some(id) = conn
            .query_row(
                "SELECT id FROM playlists WHERE tg_user_id = ?1 AND kind = 'liked'",
                params![tg_user_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO playlists (tg_user_id, name, kind) VALUES (?1, 'Liked', 'liked')",
            params![tg_user_id],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn playlist_track_ids(conn: &Connection, playlist_id: i64) -> AppResult<Vec<i64>> {
        let mut stmt = conn.prepare(
            "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 \
             ORDER BY position ASC, added_at ASC",
        )?;
        let ids = stmt
            .query_map(params![playlist_id], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    pub fn playlists_bundle(&self, tg_user_id: i64) -> AppResult<PlaylistsBundle> {
        let conn = self.conn.lock().unwrap();
        let liked_id = self.ensure_liked(&conn, tg_user_id)?;
        let liked = LikedPlaylist {
            id: liked_id,
            track_ids: Self::playlist_track_ids(&conn, liked_id)?,
            updated_at: Self::playlist_updated_at(&conn, liked_id)?,
        };

        let mut stmt = conn.prepare(
            "SELECT id, name, thumbnail_data, thumbnail_mime, updated_at FROM playlists \
             WHERE tg_user_id = ?1 AND kind = 'custom' ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map(params![tg_user_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut custom = Vec::with_capacity(rows.len());
        for (id, name, thumb_data, thumb_mime, updated_at) in rows {
            custom.push(CustomPlaylistSummary {
                id,
                name,
                track_ids: Self::playlist_track_ids(&conn, id)?,
                has_thumbnail: thumb_data.is_some() && thumb_mime.is_some(),
                updated_at,
            });
        }

        Ok(PlaylistsBundle { liked, custom })
    }

    pub fn create_playlist(
        &self,
        tg_user_id: i64,
        name: &str,
        thumbnail: Option<(&str, &str)>,
    ) -> AppResult<CustomPlaylistSummary> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(crate::error::AppError::msg("Playlist name is required"));
        }
        let conn = self.conn.lock().unwrap();
        let (data, mime) = match thumbnail {
            Some((d, m)) => (Some(d), Some(m)),
            None => (None, None),
        };
        conn.execute(
            "INSERT INTO playlists (tg_user_id, name, kind, thumbnail_data, thumbnail_mime) \
             VALUES (?1, ?2, 'custom', ?3, ?4)",
            params![tg_user_id, trimmed, data, mime],
        )?;
        let id = conn.last_insert_rowid();
        Ok(CustomPlaylistSummary {
            id,
            name: trimmed.to_string(),
            track_ids: Vec::new(),
            has_thumbnail: data.is_some(),
            updated_at: Self::playlist_updated_at(&conn, id)?,
        })
    }

    pub fn update_playlist(
        &self,
        id: i64,
        tg_user_id: i64,
        name: Option<&str>,
        // None = leave as-is, Some(None) = clear, Some(Some(..)) = replace
        thumbnail: Option<Option<(&str, &str)>>,
    ) -> AppResult<CustomPlaylistSummary> {
        let conn = self.conn.lock().unwrap();
        let existing = conn
            .query_row(
                "SELECT name, kind, thumbnail_data, thumbnail_mime FROM playlists \
                 WHERE id = ?1 AND tg_user_id = ?2",
                params![id, tg_user_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| crate::error::AppError::msg("Playlist not found"))?;

        if existing.1 != "custom" {
            return Err(crate::error::AppError::msg(
                "Only custom playlists can be edited",
            ));
        }

        let next_name = match name {
            Some(n) => {
                let trimmed = n.trim();
                if trimmed.is_empty() {
                    return Err(crate::error::AppError::msg("Playlist name is required"));
                }
                trimmed.to_string()
            }
            None => existing.0,
        };

        let (next_data, next_mime): (Option<String>, Option<String>) = match thumbnail {
            None => (existing.2, existing.3),
            Some(None) => (None, None),
            Some(Some((d, m))) => (Some(d.to_string()), Some(m.to_string())),
        };

        conn.execute(
            "UPDATE playlists SET name = ?1, thumbnail_data = ?2, thumbnail_mime = ?3, \
             updated_at = datetime('now') \
             WHERE id = ?4 AND tg_user_id = ?5",
            params![next_name, next_data, next_mime, id, tg_user_id],
        )?;

        Ok(CustomPlaylistSummary {
            id,
            name: next_name,
            track_ids: Self::playlist_track_ids(&conn, id)?,
            has_thumbnail: next_data.is_some() && next_mime.is_some(),
            updated_at: Self::playlist_updated_at(&conn, id)?,
        })
    }

    pub fn playlist_thumbnail(
        &self,
        id: i64,
        tg_user_id: i64,
    ) -> AppResult<Option<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT thumbnail_data, thumbnail_mime FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
                params![id, tg_user_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .optional()?;
        Ok(row.and_then(|(d, m)| match (d, m) {
            (Some(d), Some(m)) => Some((d, m)),
            _ => None,
        }))
    }

    pub fn delete_playlist(&self, id: i64, tg_user_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let kind = conn
            .query_row(
                "SELECT kind FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
                params![id, tg_user_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| crate::error::AppError::msg("Playlist not found"))?;
        if kind == "liked" {
            return Err(crate::error::AppError::msg(
                "Cannot delete the Liked playlist",
            ));
        }
        conn.execute(
            "DELETE FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
            params![id, tg_user_id],
        )?;
        Ok(())
    }

    fn next_position(conn: &Connection, playlist_id: i64) -> AppResult<i64> {
        let pos = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(pos)
    }

    pub fn add_track_to_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
        tg_user_id: i64,
    ) -> AppResult<String> {
        let conn = self.conn.lock().unwrap();
        let kind = conn
            .query_row(
                "SELECT kind FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
                params![playlist_id, tg_user_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| crate::error::AppError::msg("Playlist not found"))?;
        if kind == "liked" {
            return Err(crate::error::AppError::msg(
                "Use toggle_like for the Liked playlist",
            ));
        }
        let position = Self::next_position(&conn, playlist_id)?;
        conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, position],
        )?;
        Self::touch_playlist_updated_at(&conn, playlist_id)
    }

    /// Appends tracks in order (duplicates allowed). Used when saving a queue as a playlist.
    pub fn add_tracks_to_playlist(
        &self,
        playlist_id: i64,
        track_ids: &[i64],
        tg_user_id: i64,
    ) -> AppResult<String> {
        let mut conn = self.conn.lock().unwrap();
        let kind = conn
            .query_row(
                "SELECT kind FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
                params![playlist_id, tg_user_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| crate::error::AppError::msg("Playlist not found"))?;
        if kind == "liked" {
            return Err(crate::error::AppError::msg(
                "Use toggle_like for the Liked playlist",
            ));
        }
        if track_ids.is_empty() {
            return Self::playlist_updated_at(&conn, playlist_id);
        }

        let tx = conn.transaction()?;
        let mut position = Self::next_position(&tx, playlist_id)?;
        for track_id in track_ids {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist_id, track_id, position],
            )?;
            position += 1;
        }
        let updated_at = Self::touch_playlist_updated_at(&tx, playlist_id)?;
        tx.commit()?;
        Ok(updated_at)
    }

    /// Removes the membership entry at `position` (0-based order in the playlist).
    pub fn remove_track_from_playlist(
        &self,
        playlist_id: i64,
        position: i64,
        tg_user_id: i64,
    ) -> AppResult<String> {
        let conn = self.conn.lock().unwrap();
        let exists = conn
            .query_row(
                "SELECT 1 FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
                params![playlist_id, tg_user_id],
                |_| Ok(()),
            )
            .optional()?;
        if exists.is_none() {
            return Err(crate::error::AppError::msg("Playlist not found"));
        }
        if position < 0 {
            return Err(crate::error::AppError::msg("Invalid playlist position"));
        }
        let entry_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM playlist_tracks WHERE playlist_id = ?1 \
                 ORDER BY position ASC, added_at ASC, id ASC \
                 LIMIT 1 OFFSET ?2",
                params![playlist_id, position],
                |row| row.get(0),
            )
            .optional()?;
        let Some(entry_id) = entry_id else {
            return Self::playlist_updated_at(&conn, playlist_id);
        };
        conn.execute(
            "DELETE FROM playlist_tracks WHERE id = ?1",
            params![entry_id],
        )?;
        Self::touch_playlist_updated_at(&conn, playlist_id)
    }

    /// Rewrites membership to match `track_ids` order (duplicates allowed).
    /// `track_ids` must be a multiset-equal permutation of the playlist's current membership.
    pub fn reorder_playlist_tracks(
        &self,
        playlist_id: i64,
        track_ids: &[i64],
        tg_user_id: i64,
    ) -> AppResult<String> {
        let mut conn = self.conn.lock().unwrap();
        let exists = conn
            .query_row(
                "SELECT 1 FROM playlists WHERE id = ?1 AND tg_user_id = ?2",
                params![playlist_id, tg_user_id],
                |_| Ok(()),
            )
            .optional()?;
        if exists.is_none() {
            return Err(crate::error::AppError::msg("Playlist not found"));
        }

        let current = Self::playlist_track_ids(&conn, playlist_id)?;
        if !Self::track_id_multisets_equal(&current, track_ids) {
            return Err(crate::error::AppError::msg(
                "Track list does not match playlist membership",
            ));
        }

        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
        )?;
        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist_id, track_id, position as i64],
            )?;
        }
        let updated_at = Self::touch_playlist_updated_at(&tx, playlist_id)?;
        tx.commit()?;
        Ok(updated_at)
    }

    fn track_id_multisets_equal(a: &[i64], b: &[i64]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut left = a.to_vec();
        let mut right = b.to_vec();
        left.sort_unstable();
        right.sort_unstable();
        left == right
    }

    /// Toggles the liked state of a track; returns the updated liked playlist.
    pub fn toggle_like(&self, track_id: i64, tg_user_id: i64) -> AppResult<LikedPlaylist> {
        let conn = self.conn.lock().unwrap();
        let liked_id = self.ensure_liked(&conn, tg_user_id)?;
        let already: Option<()> = conn
            .query_row(
                "SELECT 1 FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                params![liked_id, track_id],
                |_| Ok(()),
            )
            .optional()?;

        if already.is_some() {
            conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
                params![liked_id, track_id],
            )?;
        } else {
            let position = Self::next_position(&conn, liked_id)?;
            conn.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![liked_id, track_id, position],
            )?;
        }

        let updated_at = Self::touch_playlist_updated_at(&conn, liked_id)?;
        Ok(LikedPlaylist {
            id: liked_id,
            track_ids: Self::playlist_track_ids(&conn, liked_id)?,
            updated_at,
        })
    }

    // ---- listen statistics -----------------------------------------------

    fn now_ms() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    pub fn record_attempt_start(&self, track_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let ts = Self::now_ms();
        conn.execute(
            "INSERT INTO listen_events (track_id, event_type, ts_ms) VALUES (?1, 'attempt_start', ?2)",
            params![track_id, ts],
        )?;
        Ok(())
    }

    pub fn record_attempt_end(
        &self,
        track_id: i64,
        listened_ms: i64,
        duration_ms: Option<i64>,
        end_reason: EndReason,
    ) -> AppResult<ListenEndResult> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let ended_at_ms = Self::now_ms();
        let input = AttemptEndInput {
            listened_ms,
            duration_ms,
            end_reason,
            ended_at_ms,
        };
        let derived = derive_attempt_end(input);

        tx.execute(
            "INSERT INTO listen_events \
             (track_id, event_type, ts_ms, listened_ms, duration_ms, end_reason, qualified, early_skip) \
             VALUES (?1, 'attempt_end', ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                track_id,
                ended_at_ms,
                derived.listened_eff_ms,
                duration_ms,
                end_reason.as_str(),
                derived.qualified as i64,
                derived.early_skip as i64,
            ],
        )?;

        let prev = Self::load_aggregates(&tx, track_id)?;
        let next = apply_attempt_end(prev, derived, end_reason, ended_at_ms);
        let likeness = compute_likeness(&next, ended_at_ms);
        Self::upsert_aggregates(&tx, track_id, &next, likeness)?;
        tx.commit()?;

        Ok(ListenEndResult {
            qualified: derived.qualified,
            early_skip: derived.early_skip,
            listened_eff_ms: derived.listened_eff_ms,
            stats: TrackListenStats {
                track_id,
                starts: next.starts,
                qualified_plays: next.qualified_plays,
                completes: next.completes,
                early_skips: next.early_skips,
                total_listened_ms: next.total_listened_ms,
                first_played_at_ms: next.first_played_at_ms,
                last_played_at_ms: next.last_played_at_ms,
                likeness,
            },
        })
    }

    pub fn track_listen_stats(&self, track_id: i64) -> AppResult<Option<TrackListenStats>> {
        let conn = self.conn.lock().unwrap();
        let mut row = match Self::load_stats_row(&conn, track_id)? {
            Some(s) => s,
            None => return Ok(None),
        };
        let now = Self::now_ms();
        let agg = ListenAggregates {
            starts: row.starts,
            qualified_plays: row.qualified_plays,
            completes: row.completes,
            early_skips: row.early_skips,
            total_listened_ms: row.total_listened_ms,
            first_played_at_ms: row.first_played_at_ms,
            last_played_at_ms: row.last_played_at_ms,
        };
        row.likeness = compute_likeness(&agg, now);
        Ok(Some(row))
    }

    pub fn all_listen_stats(&self) -> AppResult<Vec<TrackListenStats>> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now_ms();
        let mut stmt = conn.prepare(
            "SELECT track_id, starts, qualified_plays, completes, early_skips, \
             total_listened_ms, first_played_at_ms, last_played_at_ms, likeness \
             FROM track_listen_stats ORDER BY track_id",
        )?;
        let rows = stmt
            .query_map([], |row| Self::map_stats_row(row))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .map(|mut s| {
                let agg = ListenAggregates {
                    starts: s.starts,
                    qualified_plays: s.qualified_plays,
                    completes: s.completes,
                    early_skips: s.early_skips,
                    total_listened_ms: s.total_listened_ms,
                    first_played_at_ms: s.first_played_at_ms,
                    last_played_at_ms: s.last_played_at_ms,
                };
                s.likeness = compute_likeness(&agg, now);
                s
            })
            .collect())
    }

    pub fn rebuild_listen_stats(&self) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch("DELETE FROM track_listen_stats;")?;

        let mut stmt = tx.prepare(
            "SELECT track_id, listened_ms, duration_ms, end_reason, ts_ms \
             FROM listen_events WHERE event_type = 'attempt_end' ORDER BY id ASC",
        )?;
        let events: Vec<(i64, i64, Option<i64>, String, i64)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut by_track: std::collections::HashMap<i64, ListenAggregates> =
            std::collections::HashMap::new();

        for (track_id, listened_ms, duration_ms, end_reason_str, ended_at_ms) in events {
            let Some(end_reason) = EndReason::parse(&end_reason_str) else {
                continue;
            };
            let derived = derive_attempt_end(AttemptEndInput {
                listened_ms,
                duration_ms,
                end_reason,
                ended_at_ms,
            });
            let prev = by_track.remove(&track_id).unwrap_or_default();
            let next = apply_attempt_end(prev, derived, end_reason, ended_at_ms);
            by_track.insert(track_id, next);
        }

        let now = Self::now_ms();
        for (track_id, agg) in by_track {
            let likeness = compute_likeness(&agg, now);
            Self::upsert_aggregates(&tx, track_id, &agg, likeness)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn load_aggregates(conn: &Connection, track_id: i64) -> AppResult<ListenAggregates> {
        Ok(conn
            .query_row(
                "SELECT starts, qualified_plays, completes, early_skips, \
                 total_listened_ms, first_played_at_ms, last_played_at_ms \
                 FROM track_listen_stats WHERE track_id = ?1",
                params![track_id],
                |row| {
                    Ok(ListenAggregates {
                        starts: row.get(0)?,
                        qualified_plays: row.get(1)?,
                        completes: row.get(2)?,
                        early_skips: row.get(3)?,
                        total_listened_ms: row.get(4)?,
                        first_played_at_ms: row.get(5)?,
                        last_played_at_ms: row.get(6)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }

    fn upsert_aggregates(
        conn: &Connection,
        track_id: i64,
        agg: &ListenAggregates,
        likeness: f64,
    ) -> AppResult<()> {
        conn.execute(
            "INSERT INTO track_listen_stats \
             (track_id, starts, qualified_plays, completes, early_skips, \
              total_listened_ms, first_played_at_ms, last_played_at_ms, likeness) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(track_id) DO UPDATE SET \
               starts = excluded.starts, \
               qualified_plays = excluded.qualified_plays, \
               completes = excluded.completes, \
               early_skips = excluded.early_skips, \
               total_listened_ms = excluded.total_listened_ms, \
               first_played_at_ms = excluded.first_played_at_ms, \
               last_played_at_ms = excluded.last_played_at_ms, \
               likeness = excluded.likeness",
            params![
                track_id,
                agg.starts,
                agg.qualified_plays,
                agg.completes,
                agg.early_skips,
                agg.total_listened_ms,
                agg.first_played_at_ms,
                agg.last_played_at_ms,
                likeness,
            ],
        )?;
        Ok(())
    }

    fn load_stats_row(conn: &Connection, track_id: i64) -> AppResult<Option<TrackListenStats>> {
        Ok(conn
            .query_row(
                "SELECT track_id, starts, qualified_plays, completes, early_skips, \
                 total_listened_ms, first_played_at_ms, last_played_at_ms, likeness \
                 FROM track_listen_stats WHERE track_id = ?1",
                params![track_id],
                Self::map_stats_row,
            )
            .optional()?)
    }

    fn map_stats_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TrackListenStats> {
        Ok(TrackListenStats {
            track_id: row.get(0)?,
            starts: row.get(1)?,
            qualified_plays: row.get(2)?,
            completes: row.get(3)?,
            early_skips: row.get(4)?,
            total_listened_ms: row.get(5)?,
            first_played_at_ms: row.get(6)?,
            last_played_at_ms: row.get(7)?,
            likeness: row.get(8)?,
        })
    }

    // ---- bounce profiles ------------------------------------------------

    pub fn track_bounce_profile(
        &self,
        track_id: i64,
    ) -> AppResult<Option<TrackBounceProfileRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT track_id, algorithm_version, frame_ms, duration_ms, file_size, loudness, onset
             FROM track_bounce_profiles WHERE track_id = ?1",
            params![track_id],
            |row| {
                Ok(TrackBounceProfileRecord {
                    track_id: row.get(0)?,
                    algorithm_version: row.get(1)?,
                    frame_ms: row.get(2)?,
                    duration_ms: row.get(3)?,
                    file_size: row.get(4)?,
                    loudness: row.get(5)?,
                    onset: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn save_track_bounce_profile(
        &self,
        profile: &TrackBounceProfileRecord,
    ) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO track_bounce_profiles
               (track_id, algorithm_version, frame_ms, duration_ms, file_size, loudness, onset)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(track_id) DO UPDATE SET
               algorithm_version = excluded.algorithm_version,
               frame_ms = excluded.frame_ms,
               duration_ms = excluded.duration_ms,
               file_size = excluded.file_size,
               loudness = excluded.loudness,
               onset = excluded.onset,
               created_at = datetime('now')",
            params![
                profile.track_id,
                profile.algorithm_version,
                profile.frame_ms,
                profile.duration_ms,
                profile.file_size,
                profile.loudness,
                profile.onset,
            ],
        )?;
        Ok(())
    }

    // ---- app settings ---------------------------------------------------

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

/// Default audio cache size limit: 5 GiB.
pub const DEFAULT_CACHE_LIMIT_BYTES: i64 = 5_368_709_120;
/// Default cache TTL: 30 days.
pub const DEFAULT_CACHE_TTL_SECS: i64 = 2_592_000;

pub const SETTING_CACHE_LIMIT_BYTES: &str = "cache_limit_bytes";
pub const SETTING_CACHE_TTL_SECS: &str = "cache_ttl_secs";

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    #[serde(rename = "tgUserId")]
    pub tg_user_id: i64,
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}

fn map_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        tg_user_id: row.get(1)?,
        file_id: row.get(2)?,
        file_unique_id: row.get(3)?,
        title: row.get(4)?,
        performer: row.get(5)?,
        duration: row.get(6)?,
        source: row.get(7)?,
        mime_type: row.get(8)?,
        file_size: row.get(9)?,
        created_at: row.get(10)?,
        mtproto_document: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolated DB per test. File-backed temp paths keyed only by pid+nanos
    /// collided under the parallel test harness (`database is locked`).
    fn test_db() -> AppResult<Db> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Db::apply_schema(&conn)?;
        Ok(Db {
            conn: Mutex::new(conn),
        })
    }

    #[test]
    fn load_profile_includes_phone() -> AppResult<()> {
        let db = test_db()?;
        db.save_profile(42, "Ada", Some("Lovelace"), Some("ada"), Some("+1555"))?;
        let profile = db.load_profile()?.expect("profile");
        assert_eq!(profile.tg_user_id, 42);
        assert_eq!(profile.first_name, "Ada");
        assert_eq!(profile.last_name.as_deref(), Some("Lovelace"));
        assert_eq!(profile.username.as_deref(), Some("ada"));
        assert_eq!(profile.phone.as_deref(), Some("+1555"));
        Ok(())
    }

    #[test]
    fn bounce_profile_round_trips_and_cascades_with_track() -> AppResult<()> {
        let db = test_db()?;
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO tracks
                   (id, tg_user_id, file_id, file_unique_id, source, file_size)
                 VALUES (7, 42, 'file', 'unique', 'mtproto', 1234)",
                [],
            )?;
        }
        let expected = TrackBounceProfileRecord {
            track_id: 7,
            algorithm_version: 3,
            frame_ms: 50,
            duration_ms: 10_000,
            file_size: Some(1234),
            loudness: vec![1, 2, 3],
            onset: vec![4, 5, 6],
        };
        db.save_track_bounce_profile(&expected)?;
        let loaded = db.track_bounce_profile(7)?.expect("profile");
        assert_eq!(loaded.algorithm_version, expected.algorithm_version);
        assert_eq!(loaded.loudness, expected.loudness);
        assert_eq!(loaded.onset, expected.onset);

        db.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM tracks WHERE id = 7", [])?;
        assert!(db.track_bounce_profile(7)?.is_none());
        Ok(())
    }

    #[test]
    fn clearing_active_profile_keeps_user_scoped_playlists() -> AppResult<()> {
        let db = test_db()?;
        let user_a = 1001;
        let user_c = 2002;

        db.save_profile(user_a, "User A", None, Some("user_a"), None)?;
        let playlist = db.create_playlist(user_a, "Playlist B", None)?;
        db.clear_active_profile(user_a)?;
        assert!(db.load_profile()?.is_none());

        db.save_profile(user_c, "User C", None, Some("user_c"), None)?;
        let user_c_playlists = db.playlists_bundle(user_c)?;
        assert!(user_c_playlists.custom.is_empty());

        db.clear_active_profile(user_c)?;
        db.save_profile(user_a, "User A", None, Some("user_a"), None)?;
        let user_a_playlists = db.playlists_bundle(user_a)?;

        assert_eq!(user_a_playlists.custom.len(), 1);
        assert_eq!(user_a_playlists.custom[0].id, playlist.id);
        assert_eq!(user_a_playlists.custom[0].name, "Playlist B");
        Ok(())
    }

    #[test]
    fn listen_attempt_updates_aggregates_and_rebuild_matches() -> AppResult<()> {
        use crate::listen_stats::EndReason;

        let db = test_db()?;
        let track_id = 42;

        db.record_attempt_start(track_id)?;
        let end = db.record_attempt_end(
            track_id,
            120_000,
            Some(180_000),
            EndReason::Completed,
        )?;
        assert!(end.qualified);
        assert!(!end.early_skip);
        assert_eq!(end.stats.starts, 1);
        assert_eq!(end.stats.completes, 1);
        assert_eq!(end.stats.qualified_plays, 1);
        assert!(end.stats.likeness > 0.0);

        db.record_attempt_start(track_id)?;
        let skip = db.record_attempt_end(
            track_id,
            5_000,
            Some(180_000),
            EndReason::Skipped,
        )?;
        assert!(!skip.qualified);
        assert!(skip.early_skip);
        assert_eq!(skip.stats.starts, 2);
        assert_eq!(skip.stats.early_skips, 1);

        let before = db.track_listen_stats(track_id)?.expect("stats row");
        db.rebuild_listen_stats()?;
        let after = db.track_listen_stats(track_id)?.expect("stats after rebuild");

        assert_eq!(before.starts, after.starts);
        assert_eq!(before.qualified_plays, after.qualified_plays);
        assert_eq!(before.completes, after.completes);
        assert_eq!(before.early_skips, after.early_skips);
        assert_eq!(before.total_listened_ms, after.total_listened_ms);
        Ok(())
    }

    #[test]
    fn listen_events_survive_without_track_row() -> AppResult<()> {
        use crate::listen_stats::EndReason;

        let db = test_db()?;
        // Orphan track id — no FK to tracks.
        db.record_attempt_start(999)?;
        let end = db.record_attempt_end(999, 40_000, Some(60_000), EndReason::Stopped)?;
        assert!(end.qualified);
        assert!(db.track_listen_stats(999)?.is_some());
        Ok(())
    }

    fn upsert_test_track(db: &Db, user: i64, unique: &str, position: i64) -> AppResult<i64> {
        db.upsert_track(&UpsertTrack {
            tg_user_id: user,
            file_id: format!("file-{unique}"),
            file_unique_id: unique.to_string(),
            title: Some(unique.to_string()),
            performer: None,
            duration: Some(60),
            mime_type: Some("audio/mpeg".into()),
            file_size: Some(1000),
            track_position: position,
            mtproto_document: "{}".into(),
        })?;
        let tracks = db.tracks_by_user(user)?;
        tracks
            .into_iter()
            .find(|t| t.file_unique_id == unique)
            .map(|t| t.id)
            .ok_or_else(|| crate::error::AppError::msg("track not found after upsert"))
    }

    #[test]
    fn reorder_playlist_tracks_persists_duplicate_order() -> AppResult<()> {
        let db = test_db()?;
        let user = 42;
        db.save_profile(user, "User", None, None, None)?;
        let a = upsert_test_track(&db, user, "dup-a", 0)?;
        let b = upsert_test_track(&db, user, "dup-b", 1)?;
        let playlist = db.create_playlist(user, "Dupes", None)?;
        db.add_tracks_to_playlist(playlist.id, &[a, b, a], user)?;

        let before = db.playlists_bundle(user)?;
        assert_eq!(before.custom[0].track_ids, vec![a, b, a]);

        db.reorder_playlist_tracks(playlist.id, &[b, a, a], user)?;
        let after = db.playlists_bundle(user)?;
        assert_eq!(after.custom[0].track_ids, vec![b, a, a]);

        db.reorder_playlist_tracks(playlist.id, &[a, a, b], user)?;
        let again = db.playlists_bundle(user)?;
        assert_eq!(again.custom[0].track_ids, vec![a, a, b]);
        Ok(())
    }

    #[test]
    fn cache_settings_default_and_round_trip() -> AppResult<()> {
        let db = test_db()?;

        assert_eq!(
            db.get_setting_i64(SETTING_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES)?,
            DEFAULT_CACHE_LIMIT_BYTES
        );
        assert_eq!(
            db.get_setting_i64(SETTING_CACHE_TTL_SECS, DEFAULT_CACHE_TTL_SECS)?,
            DEFAULT_CACHE_TTL_SECS
        );

        db.set_setting(SETTING_CACHE_LIMIT_BYTES, "1073741824")?;
        db.set_setting(SETTING_CACHE_TTL_SECS, "86400")?;

        assert_eq!(
            db.get_setting_i64(SETTING_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES)?,
            1_073_741_824
        );
        assert_eq!(
            db.get_setting_i64(SETTING_CACHE_TTL_SECS, DEFAULT_CACHE_TTL_SECS)?,
            86_400
        );
        assert_eq!(
            db.get_setting(SETTING_CACHE_LIMIT_BYTES)?.as_deref(),
            Some("1073741824")
        );
        Ok(())
    }

    #[test]
    fn cache_settings_invalid_int_falls_back_to_default() -> AppResult<()> {
        let db = test_db()?;
        db.set_setting(SETTING_CACHE_LIMIT_BYTES, "not-a-number")?;
        assert_eq!(
            db.get_setting_i64(SETTING_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES)?,
            DEFAULT_CACHE_LIMIT_BYTES
        );
        Ok(())
    }
}
