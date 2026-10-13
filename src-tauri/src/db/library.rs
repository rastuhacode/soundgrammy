//! Bounded display reads and durable saved-music scan checkpoints.

use rusqlite::{params, Connection, OptionalExtension};

use super::{
    tracks::{map_track, upsert_on},
    Db, LibraryBatch, LibraryPage, LibraryStatus, PositionedTrack, SavedMusicCheckpoint,
    UpsertTrack,
};
use crate::error::{AppError, AppResult};

// Display reads deliberately avoid loading Telegram document JSON.
const DISPLAY_COLUMNS: &str = "id, tg_user_id, file_id, file_unique_id, title, title_source, performer, duration, source, mime_type, file_size, created_at, NULL, track_position";

fn positioned(row: &rusqlite::Row<'_>) -> rusqlite::Result<PositionedTrack> {
    Ok(PositionedTrack {
        track: map_track(row)?,
        position: row.get(13)?,
    })
}

fn active_user(conn: &Connection, uid: i64) -> AppResult<()> {
    let active = conn
        .query_row("SELECT tg_user_id FROM profile LIMIT 1", [], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?;
    if active != Some(uid) {
        return Err(AppError::NotAuthorized);
    }
    Ok(())
}

fn status(conn: &Connection, uid: i64) -> AppResult<LibraryStatus> {
    let state = conn
        .query_row(
            "SELECT revision, complete, blocking, next_offset FROM library_sync_state WHERE tg_user_id = ?1",
            [uid],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?, r.get::<_, bool>(2)?, r.get::<_, i64>(3)?)),
        )
        .optional()?;
    let last_sync_at = conn
        .query_row(
            "SELECT last_sync_at FROM app_meta WHERE tg_user_id = ?1",
            [uid],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    let (indexed, total) = if state.is_some() {
        conn.query_row(
            "SELECT COUNT(t.id), COUNT(m.document_id) FROM library_sync_manifest m \
             LEFT JOIN tracks t ON t.file_unique_id = m.document_id AND t.tg_user_id = m.tg_user_id \
             WHERE m.tg_user_id = ?1", [uid], |r| Ok((r.get(0)?, r.get(1)?)),
        )?
    } else {
        let count = conn.query_row(
            "SELECT COUNT(*) FROM tracks WHERE tg_user_id = ?1",
            [uid],
            |r| r.get::<_, i64>(0),
        )?;
        (count, count)
    };
    Ok(LibraryStatus {
        tg_user_id: uid,
        revision: state.map_or(0, |s| s.0),
        indexed,
        total,
        complete: state.map_or(last_sync_at.is_some(), |s| s.1),
        blocking: state.is_some_and(|s| s.2)
            || (last_sync_at.is_none() && !state.is_some_and(|s| s.1)),
        processed: state.map_or(0, |s| s.3),
        last_sync_at,
    })
}

impl Db {
    pub fn defer_saved_music_sync(&self, uid: i64, retry_at: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        active_user(&conn, uid)?;
        conn.execute(
            "INSERT INTO library_sync_backoff VALUES (?1, ?2) \
            ON CONFLICT(tg_user_id) DO UPDATE SET retry_at = MAX(retry_at, excluded.retry_at)",
            params![uid, retry_at],
        )?;
        Ok(())
    }

    pub fn saved_music_wait_seconds(&self, uid: i64, now: i64) -> AppResult<i64> {
        let conn = self.conn.lock().unwrap();
        let retry_at = conn
            .query_row(
                "SELECT retry_at FROM library_sync_backoff WHERE tg_user_id = ?1",
                [uid],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        Ok(retry_at.map_or(0, |at| at.saturating_sub(now).max(0)))
    }

    pub fn library_status(&self, uid: i64) -> AppResult<LibraryStatus> {
        status(&self.conn.lock().unwrap(), uid)
    }

    /// One revision for every page prevents mixed snapshots during background sync.
    pub fn tracks_page(
        &self,
        uid: i64,
        offset: i64,
        limit: i64,
        revision: Option<i64>,
    ) -> AppResult<LibraryPage> {
        if offset < 0 || !(1..=500).contains(&limit) {
            return Err(AppError::msg(
                "Library page requires a nonnegative offset and limit between 1 and 500",
            ));
        }
        let conn = self.conn.lock().unwrap();
        let status = status(&conn, uid)?;
        if revision.is_some_and(|r| r != status.revision) {
            return Err(AppError::msg("Library changed while loading pages; retry"));
        }
        if status.blocking {
            return Ok(LibraryPage {
                tracks: vec![],
                status,
                next_offset: None,
            });
        }
        let mut stmt = conn.prepare(&format!(
            "SELECT {DISPLAY_COLUMNS} FROM tracks WHERE tg_user_id = ?1 \
             ORDER BY track_position ASC, created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
        ))?;
        // Fetch one extra row to avoid a trailing empty IPC request.
        let mut tracks = stmt
            .query_map(params![uid, limit + 1, offset], positioned)?
            .collect::<Result<Vec<_>, _>>()?;
        let next_offset = if tracks.len() > limit as usize {
            tracks.pop();
            Some(offset + limit)
        } else {
            None
        };
        Ok(LibraryPage {
            tracks,
            status,
            next_offset,
        })
    }

    pub fn saved_music_checkpoint(&self, uid: i64) -> AppResult<Option<SavedMusicCheckpoint>> {
        let conn = self.conn.lock().unwrap();
        let state = conn.query_row(
            "SELECT manifest_hash, next_offset, complete, blocking FROM library_sync_state WHERE tg_user_id = ?1", [uid],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?, r.get::<_, bool>(2)?, r.get::<_, bool>(3)?)),
        ).optional()?;
        let Some((hash, next_offset, complete, blocking)) = state else {
            return Ok(None);
        };
        let mut stmt = conn.prepare(
            "SELECT document_id FROM library_sync_manifest WHERE tg_user_id = ?1 ORDER BY ordinal",
        )?;
        let strings = stmt
            .query_map([uid], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let manifest = strings
            .into_iter()
            .map(|id| {
                id.parse::<i64>()
                    .map_err(|_| AppError::msg("Invalid saved-music checkpoint"))
            })
            .collect::<AppResult<Vec<_>>>()?;
        Ok(Some(SavedMusicCheckpoint {
            manifest,
            hash: hash
                .parse()
                .map_err(|_| AppError::msg("Invalid saved-music hash"))?,
            next_offset,
            complete,
            blocking,
        }))
    }

    /// The manifest is durable before fetching metadata. Its hash alone never
    /// marks an interrupted metadata scan complete.
    pub fn begin_saved_music_scan(
        &self,
        uid: i64,
        ids: &[i64],
        hash: i64,
    ) -> AppResult<LibraryBatch> {
        self.begin_saved_music_scan_mode(uid, ids, hash, true)
    }

    pub fn begin_saved_music_scan_mode(
        &self,
        uid: i64,
        ids: &[i64],
        hash: i64,
        blocking: bool,
    ) -> AppResult<LibraryBatch> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        active_user(&tx, uid)?;
        tx.execute("INSERT INTO library_sync_state (tg_user_id, manifest_hash, revision, blocking) VALUES (?1, ?2, 1, ?3) \
            ON CONFLICT(tg_user_id) DO UPDATE SET manifest_hash = excluded.manifest_hash, next_offset = 0, complete = 0, blocking = excluded.blocking, revision = revision + 1", params![uid, hash.to_string(), blocking])?;
        tx.execute(
            "DELETE FROM library_sync_manifest WHERE tg_user_id = ?1",
            [uid],
        )?;
        tx.execute(
            "DELETE FROM library_sync_entries WHERE tg_user_id = ?1",
            [uid],
        )?;
        {
            let mut stmt = tx.prepare("INSERT INTO library_sync_manifest VALUES (?1, ?2, ?3)")?;
            for (ordinal, id) in ids.iter().enumerate() {
                stmt.execute(params![uid, id.to_string(), ordinal as i64])?;
            }
        }
        if !blocking {
            // Preserve the known relative order. Manifest ordinals are only
            // request hints; Telegram does not document their ordering.
            tx.execute("INSERT INTO library_sync_entries SELECT ?1, file_unique_id, ROW_NUMBER() OVER \
                (ORDER BY track_position ASC, created_at DESC, id DESC) - 1 FROM tracks \
                WHERE tg_user_id = ?1 AND source = 'mtproto' AND EXISTS \
                (SELECT 1 FROM library_sync_manifest m WHERE m.tg_user_id = ?1 AND m.document_id = tracks.file_unique_id)", [uid])?;
        }
        let status = status(&tx, uid)?;
        tx.commit()?;
        Ok(LibraryBatch {
            tracks: vec![],
            removed_track_ids: vec![],
            status,
        })
    }

    /// Record the confirmed full-reindex intent before contacting Telegram.
    /// Reuse track rows so playlists, likes, statistics and audio cache IDs survive.
    pub fn request_full_reindex(&self, uid: i64) -> AppResult<LibraryBatch> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        active_user(&tx, uid)?;
        tx.execute("INSERT INTO library_sync_state (tg_user_id, manifest_hash, blocking, revision) VALUES (?1, '0', 1, 1) \
            ON CONFLICT(tg_user_id) DO UPDATE SET complete = 0, blocking = 1, next_offset = 0, revision = revision + 1", [uid])?;
        tx.execute(
            "DELETE FROM library_sync_entries WHERE tg_user_id = ?1",
            [uid],
        )?;
        let status = status(&tx, uid)?;
        tx.commit()?;
        Ok(LibraryBatch {
            tracks: vec![],
            removed_track_ids: vec![],
            status,
        })
    }

    pub fn missing_saved_music_ids(&self, uid: i64) -> AppResult<Vec<(String, i32)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT m.document_id, m.ordinal FROM library_sync_manifest m \
            WHERE m.tg_user_id = ?1 AND NOT EXISTS \
            (SELECT 1 FROM tracks t WHERE t.tg_user_id = ?1 AND t.file_unique_id = m.document_id) ORDER BY m.ordinal")?;
        let ids = stmt
            .query_map([uid], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Only new metadata is written during a soft sync; existing rows aren't refreshed.
    pub fn append_saved_music_missing(
        &self,
        uid: i64,
        tracks: &[UpsertTrack],
    ) -> AppResult<LibraryBatch> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        active_user(&tx, uid)?;
        let mut display = vec![];
        for track in tracks {
            if track.tg_user_id != uid {
                return Err(AppError::NotAuthorized);
            }
            let known: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM tracks WHERE tg_user_id = ?1 AND file_unique_id = ?2)",
                params![uid, track.file_unique_id],
                |r| r.get(0),
            )?;
            if known {
                continue;
            }
            upsert_on(&tx, track)?;
            display.push(tx.query_row(&format!("SELECT {DISPLAY_COLUMNS} FROM tracks WHERE tg_user_id = ?1 AND file_unique_id = ?2"), params![uid, track.file_unique_id], positioned)?);
        }
        tx.execute(
            "UPDATE library_sync_state SET revision = revision + 1 WHERE tg_user_id = ?1",
            [uid],
        )?;
        let status = status(&tx, uid)?;
        tx.commit()?;
        Ok(LibraryBatch {
            tracks: display,
            removed_track_ids: vec![],
            status,
        })
    }

    /// Merge new documents at observed page positions, keeping cached relative
    /// order. This doesn't assume the manifest itself is in playlist order.
    pub fn prepare_saved_music_soft_finish(&self, uid: i64) -> AppResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        active_user(&tx, uid)?;
        let mut order = {
            let mut stmt = tx.prepare("SELECT document_id FROM library_sync_entries WHERE tg_user_id = ?1 ORDER BY position")?;
            let ids = stmt
                .query_map([uid], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        let new = {
            let mut stmt = tx.prepare("SELECT t.file_unique_id, t.track_position FROM tracks t \
                JOIN library_sync_manifest m ON m.tg_user_id = t.tg_user_id AND m.document_id = t.file_unique_id \
                WHERE t.tg_user_id = ?1 AND NOT EXISTS \
                (SELECT 1 FROM library_sync_entries e WHERE e.tg_user_id = ?1 AND e.document_id = t.file_unique_id) \
                ORDER BY t.track_position ASC, t.id DESC")?;
            let rows = stmt
                .query_map([uid], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for (id, position) in new {
            order.insert((position.max(0) as usize).min(order.len()), id);
        }
        tx.execute(
            "DELETE FROM library_sync_entries WHERE tg_user_id = ?1",
            [uid],
        )?;
        {
            let mut insert = tx.prepare("INSERT INTO library_sync_entries VALUES (?1, ?2, ?3)")?;
            for (position, id) in order.iter().enumerate() {
                insert.execute(params![uid, id, position as i64])?;
            }
        }
        tx.execute(
            "UPDATE library_sync_state SET next_offset = ?2 WHERE tg_user_id = ?1 AND blocking = 0",
            params![uid, order.len() as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn saved_music_scan_ids(&self, uid: i64, offset: i32, limit: i32) -> AppResult<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT document_id FROM library_sync_entries WHERE tg_user_id = ?1 AND position >= ?2 AND position < ?3 ORDER BY position")?;
        let strings = stmt
            .query_map(params![uid, offset, offset + limit], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        strings
            .into_iter()
            .map(|id| {
                id.parse()
                    .map_err(|_| AppError::msg("Invalid saved-music scan entry"))
            })
            .collect()
    }

    /// One short transaction commits a page, its order, and its next offset.
    /// A crash can therefore neither skip a page nor lose already-indexed tracks.
    pub fn append_saved_music_page(
        &self,
        uid: i64,
        offset: i32,
        tracks: &[UpsertTrack],
    ) -> AppResult<LibraryBatch> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        active_user(&tx, uid)?;
        let next: i32 = tx.query_row(
            "SELECT next_offset FROM library_sync_state WHERE tg_user_id = ?1 AND complete = 0",
            [uid],
            |r| r.get(0),
        )?;
        if next != offset {
            return Err(AppError::msg("Saved-music checkpoint changed"));
        }
        let mut display = vec![];
        {
            let mut seen = tx.prepare("INSERT INTO library_sync_entries VALUES (?1, ?2, ?3)")?;
            let mut select = tx.prepare(&format!("SELECT {DISPLAY_COLUMNS} FROM tracks WHERE tg_user_id = ?1 AND file_unique_id = ?2"))?;
            for (index, track) in tracks.iter().enumerate() {
                if track.tg_user_id != uid
                    || track.track_position != i64::from(offset) + index as i64
                {
                    return Err(AppError::msg(
                        "Invalid saved-music page position or account",
                    ));
                }
                seen.execute(params![uid, track.file_unique_id, track.track_position])?;
                upsert_on(&tx, track)?;
                display.push(select.query_row(params![uid, track.file_unique_id], positioned)?);
            }
        }
        tx.execute("UPDATE library_sync_state SET next_offset = ?2, revision = revision + 1 WHERE tg_user_id = ?1", params![uid, offset + tracks.len() as i32])?;
        let status = status(&tx, uid)?;
        tx.commit()?;
        Ok(LibraryBatch {
            tracks: display,
            removed_track_ids: vec![],
            status,
        })
    }

    /// Call only after a final remote manifest check. SQL also verifies exact
    /// membership; an empty/short/duplicate page cannot prune the catalogue.
    pub fn finish_saved_music_scan(&self, uid: i64) -> AppResult<LibraryBatch> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        active_user(&tx, uid)?;
        let next: i64 = tx.query_row(
            "SELECT next_offset FROM library_sync_state WHERE tg_user_id = ?1 AND complete = 0",
            [uid],
            |r| r.get(0),
        )?;
        let total: i64 = tx.query_row(
            "SELECT COUNT(*) FROM library_sync_manifest WHERE tg_user_id = ?1",
            [uid],
            |r| r.get(0),
        )?;
        if next != total {
            return Err(AppError::msg("Saved-music scan is incomplete"));
        }
        let mismatch: bool = tx.query_row(
            "SELECT EXISTS (SELECT document_id FROM library_sync_manifest WHERE tg_user_id = ?1 \
             EXCEPT SELECT document_id FROM library_sync_entries WHERE tg_user_id = ?1) \
             OR EXISTS (SELECT document_id FROM library_sync_entries WHERE tg_user_id = ?1 \
             EXCEPT SELECT document_id FROM library_sync_manifest WHERE tg_user_id = ?1)",
            [uid],
            |r| r.get(0),
        )?;
        if mismatch {
            return Err(AppError::msg("Saved music changed during indexing; retry"));
        }
        let removed_track_ids = {
            let mut stmt = tx.prepare("SELECT id FROM tracks WHERE tg_user_id = ?1 AND source = 'mtproto' AND NOT EXISTS \
                (SELECT 1 FROM library_sync_manifest m WHERE m.tg_user_id = tracks.tg_user_id AND m.document_id = tracks.file_unique_id)")?;
            let ids = stmt
                .query_map([uid], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        tx.execute("DELETE FROM tracks WHERE tg_user_id = ?1 AND source = 'mtproto' AND NOT EXISTS \
            (SELECT 1 FROM library_sync_manifest m WHERE m.tg_user_id = tracks.tg_user_id AND m.document_id = tracks.file_unique_id)", [uid])?;
        tx.execute("UPDATE tracks SET track_position = \
            (SELECT position FROM library_sync_entries e WHERE e.tg_user_id = ?1 AND e.document_id = tracks.file_unique_id) \
            WHERE tg_user_id = ?1 AND source = 'mtproto'", [uid])?;
        tx.execute("UPDATE library_sync_state SET complete = 1, blocking = 0, verified_at = unixepoch(), revision = revision + 1 WHERE tg_user_id = ?1", [uid])?;
        tx.execute(
            "INSERT INTO app_meta (tg_user_id, last_sync_at) VALUES (?1, datetime('now')) \
            ON CONFLICT(tg_user_id) DO UPDATE SET last_sync_at = excluded.last_sync_at",
            [uid],
        )?;
        let status = status(&tx, uid)?;
        tx.commit()?;
        Ok(LibraryBatch {
            tracks: vec![],
            removed_track_ids,
            status,
        })
    }
}
