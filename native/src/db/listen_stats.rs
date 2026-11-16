use rusqlite::{params, Connection, OptionalExtension};

use super::{Db, ListenEndResult, TrackListenStats};
use crate::error::AppResult;
use crate::listen_stats::{
    apply_attempt_end, compute_likeness, derive_attempt_end, AttemptEndInput, EndReason,
    ListenAggregates,
};

fn ensure_listen_identity(conn: &Connection, track_id: i64) -> AppResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO listen_track_identity(track_id,tg_user_id,file_unique_id)
        SELECT ?1,tg_user_id,file_unique_id FROM tracks WHERE id=?1",
        [track_id],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO listen_track_identity(track_id) VALUES(?1)",
        [track_id],
    )?;
    Ok(())
}

fn bump_history_revision(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_settings(key,value) VALUES('listen_history_revision','1') \
        ON CONFLICT(key) DO UPDATE SET value=CAST(value AS INTEGER)+1",
        [],
    )?;
    Ok(())
}
fn history_revision(conn: &Connection) -> AppResult<i64> {
    Ok(conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key='listen_history_revision'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

impl Db {
    pub fn record_attempt_start(&self, track_id: i64) -> AppResult<()> {
        let guard = self.conn.lock().unwrap();
        let conn = guard.unchecked_transaction()?;
        ensure_listen_identity(&conn, track_id)?;
        let ts = crate::time::now_ms();
        conn.execute(
            "INSERT INTO listen_events (track_id, event_type, ts_ms) VALUES (?1, 'attempt_start', ?2)",
            params![track_id, ts],
        )?;
        bump_history_revision(&conn)?;
        conn.commit()?;
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
        ensure_listen_identity(&tx, track_id)?;
        let ended_at_ms = crate::time::now_ms();
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
        bump_history_revision(&tx)?;
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
        let now = crate::time::now_ms();
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

    #[cfg(test)]
    pub fn all_listen_stats(&self) -> AppResult<Vec<TrackListenStats>> {
        let conn = self.conn.lock().unwrap();
        let now = crate::time::now_ms();
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

    pub fn listen_stats_for_user(&self, uid: i64) -> AppResult<Vec<TrackListenStats>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT s.track_id,s.starts,s.qualified_plays,s.completes,s.early_skips,s.total_listened_ms,s.first_played_at_ms,s.last_played_at_ms,s.likeness
            FROM track_listen_stats s JOIN listen_track_identity i ON i.track_id=s.track_id WHERE i.tg_user_id=?1 ORDER BY s.track_id")?;
        let mut rows = stmt
            .query_map([uid], Self::map_stats_row)?
            .collect::<Result<Vec<_>, _>>()?;
        let now = crate::time::now_ms();
        for s in &mut rows {
            s.likeness = compute_likeness(
                &ListenAggregates {
                    starts: s.starts,
                    qualified_plays: s.qualified_plays,
                    completes: s.completes,
                    early_skips: s.early_skips,
                    total_listened_ms: s.total_listened_ms,
                    first_played_at_ms: s.first_played_at_ms,
                    last_played_at_ms: s.last_played_at_ms,
                },
                now,
            );
        }
        Ok(rows)
    }
    pub fn clear_listen_stats_for_user(&self, uid: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM listen_events WHERE track_id IN (SELECT track_id FROM listen_track_identity WHERE tg_user_id=?1)",[uid])?;
        tx.execute("DELETE FROM track_listen_stats WHERE track_id IN (SELECT track_id FROM listen_track_identity WHERE tg_user_id=?1)",[uid])?;
        bump_history_revision(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn rebuild_listen_stats(&self) -> AppResult<()> {
        self.rebuild_listen_stats_with(|| {})
    }
    pub(super) fn rebuild_listen_stats_with(&self, before_commit: impl FnOnce()) -> AppResult<()> {
        let (revision, cutoff) = {
            let conn = self.conn.lock().unwrap();
            (
                history_revision(&conn)?,
                conn.query_row("SELECT COALESCE(MAX(id),0) FROM listen_events", [], |r| {
                    r.get::<_, i64>(0)
                })?,
            )
        };
        let mut by_track: std::collections::HashMap<i64, ListenAggregates> =
            std::collections::HashMap::new();
        let mut last_id = 0;
        loop {
            // Release the shared connection between pages and perform aggregate CPU work outside it.
            let events = {
                let conn = self.conn.lock().unwrap();
                let mut stmt=conn.prepare("SELECT id,track_id,listened_ms,duration_ms,end_reason,ts_ms
                    FROM listen_events WHERE event_type='attempt_end' AND id>?1 AND id<=?2 ORDER BY id LIMIT 1000")?;
                let events = stmt
                    .query_map(params![last_id, cutoff], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, Option<i64>>(3)?,
                            r.get::<_, String>(4)?,
                            r.get::<_, i64>(5)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                events
            };
            if events.is_empty() {
                break;
            }
            for (id, track_id, listened_ms, duration_ms, end_reason, ended_at_ms) in events {
                last_id = id;
                let Some(end_reason) = EndReason::parse(&end_reason) else {
                    continue;
                };
                let derived = derive_attempt_end(AttemptEndInput {
                    listened_ms,
                    duration_ms,
                    end_reason,
                    ended_at_ms,
                });
                let previous = by_track.remove(&track_id).unwrap_or_default();
                by_track.insert(
                    track_id,
                    apply_attempt_end(previous, derived, end_reason, ended_at_ms),
                );
            }
        }
        before_commit();
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        if history_revision(&tx)? != revision {
            return Err(crate::error::AppError::msg(
                "Listen history changed during rebuild; retry",
            ));
        }
        tx.execute("DELETE FROM track_listen_stats", [])?;
        let now = crate::time::now_ms();
        for (track_id, aggregate) in by_track {
            Self::upsert_aggregates(&tx, track_id, &aggregate, compute_likeness(&aggregate, now))?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Permanently removes both raw listen history and derived aggregates.
    #[cfg(test)]
    pub fn clear_listen_stats(&self) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch("DELETE FROM listen_events; DELETE FROM track_listen_stats;")?;
        bump_history_revision(&tx)?;
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
