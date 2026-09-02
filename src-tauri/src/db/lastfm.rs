use rusqlite::{params, OptionalExtension};

use super::Db;
use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct LastFmQueueInsert {
    pub attempt_id: String,
    pub username: String,
    pub account_key: String,
    pub track_id: Option<i64>,
    pub artist: String,
    pub track_title: String,
    pub duration_seconds: Option<i64>,
    pub started_at_utc: i64,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct LastFmQueueRow {
    pub id: i64,
    pub artist: String,
    pub track_title: String,
    pub duration_seconds: Option<i64>,
    pub started_at_utc: i64,
    pub attempt_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastFmQueueSummary {
    pub username: String,
    pub count: i64,
}

impl Db {
    pub fn enqueue_lastfm_scrobble(&self, row: &LastFmQueueInsert) -> AppResult<bool> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "INSERT OR IGNORE INTO lastfm_scrobble_queue
               (attempt_id, lastfm_username, lastfm_account_key, track_id, artist, track_title,
                duration_seconds, started_at_utc, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.attempt_id,
                row.username,
                row.account_key,
                row.track_id,
                row.artist,
                row.track_title,
                row.duration_seconds,
                row.started_at_utc,
                row.created_at_ms,
            ],
        )?;
        Ok(changed > 0)
    }

    pub fn lastfm_pending_count(&self, account_key: &str) -> AppResult<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM lastfm_scrobble_queue WHERE lastfm_account_key = ?1",
            [account_key],
            |row| row.get(0),
        )?)
    }

    pub fn lastfm_queue_summaries(&self) -> AppResult<Vec<LastFmQueueSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT MIN(lastfm_username), COUNT(*) FROM lastfm_scrobble_queue
             GROUP BY lastfm_account_key ORDER BY MIN(created_at_ms), lastfm_account_key",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(LastFmQueueSummary {
                    username: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn due_lastfm_scrobbles(
        &self,
        account_key: &str,
        now_ms: i64,
        limit: usize,
    ) -> AppResult<Vec<LastFmQueueRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, artist, track_title, duration_seconds, started_at_utc,
                    attempt_count
             FROM lastfm_scrobble_queue
             WHERE lastfm_account_key = ?1
               AND (next_attempt_at_ms IS NULL OR next_attempt_at_ms <= ?2)
             ORDER BY started_at_utc ASC, id ASC LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![account_key, now_ms, limit as i64], |row| {
                Ok(LastFmQueueRow {
                    id: row.get(0)?,
                    artist: row.get(1)?,
                    track_title: row.get(2)?,
                    duration_seconds: row.get(3)?,
                    started_at_utc: row.get(4)?,
                    attempt_count: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn next_lastfm_attempt_at(&self, account_key: &str) -> AppResult<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT MIN(next_attempt_at_ms) FROM lastfm_scrobble_queue
             WHERE lastfm_account_key = ?1 AND next_attempt_at_ms IS NOT NULL",
            [account_key],
            |row| row.get(0),
        )?)
    }

    pub fn delete_lastfm_queue_rows(&self, ids: &[i64]) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute("DELETE FROM lastfm_scrobble_queue WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn retry_lastfm_queue_rows(
        &self,
        ids: &[i64],
        next_attempt_at_ms: i64,
        code: Option<i64>,
        message: &str,
    ) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE lastfm_scrobble_queue
                 SET attempt_count = attempt_count + 1, next_attempt_at_ms = ?1,
                     last_error_code = ?2, last_error_message = ?3
                 WHERE id = ?4",
                params![next_attempt_at_ms, code, message, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_lastfm_queue_for(&self, account_key: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM lastfm_scrobble_queue WHERE lastfm_account_key = ?1",
            [account_key],
        )?;
        Ok(())
    }

    pub fn lastfm_attempt_is_queued(&self, attempt_id: &str) -> AppResult<bool> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT 1 FROM lastfm_scrobble_queue WHERE attempt_id = ?1",
                [attempt_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }
}
