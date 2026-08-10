use rusqlite::{params, Connection, OptionalExtension};

use super::{CustomPlaylistSummary, Db, LikedPlaylist, PlaylistsBundle, Track};
use crate::error::AppResult;

impl Db {
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
            .query_map(params![playlist_id], super::tracks::map_track)?
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
            "SELECT id, name, updated_at FROM playlists \
             WHERE tg_user_id = ?1 AND kind = 'custom' ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map(params![tg_user_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut custom = Vec::with_capacity(rows.len());
        for (id, name, updated_at) in rows {
            custom.push(CustomPlaylistSummary {
                id,
                name,
                track_ids: Self::playlist_track_ids(&conn, id)?,
                updated_at,
            });
        }

        Ok(PlaylistsBundle { liked, custom })
    }

    pub fn create_playlist(&self, tg_user_id: i64, name: &str) -> AppResult<CustomPlaylistSummary> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(crate::error::AppError::msg("Playlist name is required"));
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO playlists (tg_user_id, name, kind) VALUES (?1, ?2, 'custom')",
            params![tg_user_id, trimmed],
        )?;
        let id = conn.last_insert_rowid();
        Ok(CustomPlaylistSummary {
            id,
            name: trimmed.to_string(),
            track_ids: Vec::new(),
            updated_at: Self::playlist_updated_at(&conn, id)?,
        })
    }

    pub fn update_playlist(
        &self,
        id: i64,
        tg_user_id: i64,
        name: Option<&str>,
    ) -> AppResult<CustomPlaylistSummary> {
        let conn = self.conn.lock().unwrap();
        let existing = conn
            .query_row(
                "SELECT name, kind FROM playlists \
                 WHERE id = ?1 AND tg_user_id = ?2",
                params![id, tg_user_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
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

        conn.execute(
            "UPDATE playlists SET name = ?1, updated_at = datetime('now') \
             WHERE id = ?2 AND tg_user_id = ?3",
            params![next_name, id, tg_user_id],
        )?;

        Ok(CustomPlaylistSummary {
            id,
            name: next_name,
            track_ids: Self::playlist_track_ids(&conn, id)?,
            updated_at: Self::playlist_updated_at(&conn, id)?,
        })
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
        let positions = Self::next_position(&tx, playlist_id)?..;
        for (position, track_id) in positions.zip(track_ids.iter()) {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist_id, track_id, position],
            )?;
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
}
