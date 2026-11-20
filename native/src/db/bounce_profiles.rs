use rusqlite::{params, OptionalExtension};

use super::{Db, TrackBounceProfileRecord};
use crate::error::AppResult;

impl Db {
    pub fn track_bounce_profile(
        &self,
        track_id: i64,
    ) -> AppResult<Option<TrackBounceProfileRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT track_id, algorithm_version, frame_ms, duration_ms, file_size, loudness, onset, bass, mid, treble
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
                    bass: row.get(7)?,
                    mid: row.get(8)?,
                    treble: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn save_track_bounce_profile(&self, profile: &TrackBounceProfileRecord) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO track_bounce_profiles
               (track_id, algorithm_version, frame_ms, duration_ms, file_size, loudness, onset, bass, mid, treble)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(track_id) DO UPDATE SET
               algorithm_version = excluded.algorithm_version,
               frame_ms = excluded.frame_ms,
               duration_ms = excluded.duration_ms,
               file_size = excluded.file_size,
               loudness = excluded.loudness,
               onset = excluded.onset,
               bass = excluded.bass,
               mid = excluded.mid,
               treble = excluded.treble,
               created_at = datetime('now')",
            params![
                profile.track_id,
                profile.algorithm_version,
                profile.frame_ms,
                profile.duration_ms,
                profile.file_size,
                profile.loudness,
                profile.onset,
                profile.bass,
                profile.mid,
                profile.treble,
            ],
        )?;
        Ok(())
    }
}
