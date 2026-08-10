use rusqlite::{params, Connection, OptionalExtension};

use super::{Db, ListenEndResult, TrackListenStats};
use crate::error::AppResult;
use crate::listen_stats::{
    apply_attempt_end, compute_likeness, derive_attempt_end, AttemptEndInput, EndReason,
    ListenAggregates,
};

impl Db {
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
            .query_map([], Self::map_stats_row)?
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

    /// Permanently removes both raw listen history and derived aggregates.
    pub fn clear_listen_stats(&self) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch("DELETE FROM listen_events; DELETE FROM track_listen_stats;")?;
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
}
