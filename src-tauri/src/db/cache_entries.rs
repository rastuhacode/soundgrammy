use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

impl Db {
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

    pub fn clear_audio_cache_entries(&self) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM audio_cache_entries", [])?;
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
