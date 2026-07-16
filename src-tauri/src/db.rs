//! Local SQLite library (tracks + playlists), recreated from scratch.
//!
//! Ported from the web app's `lib/db.ts`, minus the session/pending-auth tables
//! (those are replaced by the encrypted session store). A single connection is
//! guarded by a `Mutex`; all access goes through small prepared-statement
//! helpers — no ORM.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::AppResult;

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
              playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
              track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
              position INTEGER NOT NULL DEFAULT 0,
              added_at TEXT NOT NULL DEFAULT (datetime('now')),
              PRIMARY KEY (playlist_id, track_id)
            );

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
            "#,
        )?;
        Self::migrate_playlists_updated_at(conn)?;
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
                "SELECT tg_user_id, first_name, last_name, username FROM profile LIMIT 1",
                [],
                |row| {
                    Ok(Profile {
                        tg_user_id: row.get(0)?,
                        first_name: row.get(1)?,
                        last_name: row.get(2)?,
                        username: row.get(3)?,
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
        let changed = conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3) \
             ON CONFLICT (playlist_id, track_id) DO NOTHING",
            params![playlist_id, track_id, position],
        )?;
        if changed > 0 {
            return Self::touch_playlist_updated_at(&conn, playlist_id);
        }
        Self::playlist_updated_at(&conn, playlist_id)
    }

    pub fn remove_track_from_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
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
        let changed = conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![playlist_id, track_id],
        )?;
        if changed > 0 {
            return Self::touch_playlist_updated_at(&conn, playlist_id);
        }
        Self::playlist_updated_at(&conn, playlist_id)
    }

    /// Rewrites `position` for every membership row to match `track_ids` order.
    /// `track_ids` must be a permutation of the playlist's current membership.
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
        if current.len() != track_ids.len() {
            return Err(crate::error::AppError::msg(
                "Track list does not match playlist membership",
            ));
        }
        let current_set: HashSet<i64> = current.into_iter().collect();
        let next_set: HashSet<i64> = track_ids.iter().copied().collect();
        if current_set != next_set {
            return Err(crate::error::AppError::msg(
                "Track list does not match playlist membership",
            ));
        }

        let tx = conn.transaction()?;
        for (position, track_id) in track_ids.iter().enumerate() {
            tx.execute(
                "UPDATE playlist_tracks SET position = ?1 \
                 WHERE playlist_id = ?2 AND track_id = ?3",
                params![position as i64, playlist_id, track_id],
            )?;
        }
        let updated_at = Self::touch_playlist_updated_at(&tx, playlist_id)?;
        tx.commit()?;
        Ok(updated_at)
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
}

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    #[serde(rename = "tgUserId")]
    pub tg_user_id: i64,
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub username: Option<String>,
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
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_db_path() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "soundgrammy-db-test-{}-{unique}.sqlite",
            std::process::id()
        ))
    }

    fn remove_sqlite_files(path: &Path) {
        let base = path.to_string_lossy();
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{base}-wal"));
        let _ = std::fs::remove_file(format!("{base}-shm"));
    }

    #[test]
    fn clearing_active_profile_keeps_user_scoped_playlists() -> AppResult<()> {
        let path = temp_db_path();

        {
            let db = Db::open(&path)?;
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
        }

        remove_sqlite_files(&path);
        Ok(())
    }
}
