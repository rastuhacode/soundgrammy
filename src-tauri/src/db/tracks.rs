use rusqlite::{params, OptionalExtension};

use super::{Db, Track, UpsertTrack};
use crate::error::AppResult;

impl Db {
    pub fn tracks_by_user(&self, tg_user_id: i64) -> AppResult<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, tg_user_id, file_id, file_unique_id, title, title_source, performer, duration, \
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
                "SELECT id, tg_user_id, file_id, file_unique_id, title, title_source, performer, duration, \
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

    pub fn upsert_track(&self, t: &UpsertTrack) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tracks (tg_user_id, file_id, file_unique_id, title, title_source, performer, duration, \
             source, mime_type, file_size, track_position, mtproto_document) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'mtproto', ?8, ?9, ?10, ?11) \
             ON CONFLICT (file_unique_id) DO UPDATE SET \
               file_id = excluded.file_id, title = excluded.title, title_source = excluded.title_source, performer = excluded.performer, \
               duration = excluded.duration, mime_type = excluded.mime_type, \
               file_size = excluded.file_size, track_position = excluded.track_position, \
               mtproto_document = excluded.mtproto_document",
            params![
                t.tg_user_id,
                t.file_id,
                t.file_unique_id,
                t.title,
                t.title_source,
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
    pub fn update_track_mime(&self, id: i64, tg_user_id: i64, mime_type: &str) -> AppResult<()> {
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
            let placeholders = std::iter::repeat_n("?", keep.len())
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
}

pub(super) fn map_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        tg_user_id: row.get(1)?,
        file_id: row.get(2)?,
        file_unique_id: row.get(3)?,
        title: row.get(4)?,
        title_source: row.get(5)?,
        performer: row.get(6)?,
        duration: row.get(7)?,
        source: row.get(8)?,
        mime_type: row.get(9)?,
        file_size: row.get(10)?,
        created_at: row.get(11)?,
        mtproto_document: row.get(12)?,
    })
}
