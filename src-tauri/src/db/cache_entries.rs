use std::collections::HashMap;

use rusqlite::params;

use super::Db;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCacheClass {
    Automatic,
    Pinned,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioCacheEntryRecord {
    pub class: AudioCacheClass,
    pub last_accessed_at_ms: i64,
}

use crate::time::now_ms;

impl Db {
    pub fn cache_identities_for_user(&self, uid: i64) -> AppResult<Vec<(i64, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut statement =
            conn.prepare("SELECT id,file_unique_id FROM tracks WHERE tg_user_id=?1")?;
        let rows = statement
            .query_map([uid], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
    pub fn cache_identities(&self) -> AppResult<Vec<(i64, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id,file_unique_id FROM tracks")?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn mark_audio_cache_automatic(&self, track_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audio_cache_entries (track_id, cache_class, last_accessed_at_ms) \
             VALUES (?1, 'automatic', ?2) \
             ON CONFLICT(track_id) DO UPDATE SET \
               last_accessed_at_ms = excluded.last_accessed_at_ms",
            params![track_id, now_ms()],
        )?;
        Ok(())
    }

    pub fn mark_audio_cache_pinned(&self, track_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audio_cache_entries (track_id, cache_class, last_accessed_at_ms) \
             VALUES (?1, 'pinned', ?2) \
             ON CONFLICT(track_id) DO UPDATE SET \
               cache_class = 'pinned', last_accessed_at_ms = excluded.last_accessed_at_ms",
            params![track_id, now_ms()],
        )?;
        Ok(())
    }

    pub fn touch_audio_cache(&self, track_id: i64) -> AppResult<()> {
        self.mark_audio_cache_automatic(track_id)
    }

    pub fn remove_audio_cache_entry(&self, track_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM audio_cache_entries WHERE track_id = ?1",
            [track_id],
        )?;
        Ok(())
    }

    pub fn audio_cache_entries(&self) -> AppResult<HashMap<i64, AudioCacheEntryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT track_id, cache_class, last_accessed_at_ms FROM audio_cache_entries",
        )?;
        let rows = stmt.query_map([], |row| {
            let class = match row.get::<_, String>(1)?.as_str() {
                "pinned" => AudioCacheClass::Pinned,
                _ => AudioCacheClass::Automatic,
            };
            Ok((
                row.get::<_, i64>(0)?,
                AudioCacheEntryRecord {
                    class,
                    last_accessed_at_ms: row.get(2)?,
                },
            ))
        })?;
        Ok(rows.collect::<Result<HashMap<_, _>, _>>()?)
    }
}
