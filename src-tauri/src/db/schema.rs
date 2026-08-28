use rusqlite::Connection;

use crate::error::AppResult;

pub(super) fn apply(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tracks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          tg_user_id INTEGER NOT NULL,
          file_id TEXT NOT NULL,
          file_unique_id TEXT NOT NULL UNIQUE,
          title TEXT,
          title_source TEXT NOT NULL DEFAULT 'filename'
            CHECK (title_source IN ('telegram_audio', 'filename', 'user_override')),
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
          created_at TEXT NOT NULL DEFAULT (datetime('now')),
          updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_playlists_user_liked
          ON playlists (tg_user_id) WHERE kind = 'liked';

        CREATE TABLE IF NOT EXISTS playlist_tracks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
          track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
          position INTEGER NOT NULL DEFAULT 0,
          added_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist_position
          ON playlist_tracks (playlist_id, position);

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

        CREATE TABLE IF NOT EXISTS app_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS listen_events (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          track_id INTEGER NOT NULL,
          event_type TEXT NOT NULL CHECK (event_type IN ('attempt_start', 'attempt_end')),
          ts_ms INTEGER NOT NULL,
          listened_ms INTEGER,
          duration_ms INTEGER,
          end_reason TEXT,
          qualified INTEGER NOT NULL DEFAULT 0,
          early_skip INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_listen_events_track_id
          ON listen_events (track_id);

        CREATE TABLE IF NOT EXISTS track_listen_stats (
          track_id INTEGER PRIMARY KEY,
          starts INTEGER NOT NULL DEFAULT 0,
          qualified_plays INTEGER NOT NULL DEFAULT 0,
          completes INTEGER NOT NULL DEFAULT 0,
          early_skips INTEGER NOT NULL DEFAULT 0,
          total_listened_ms INTEGER NOT NULL DEFAULT 0,
          first_played_at_ms INTEGER,
          last_played_at_ms INTEGER,
          likeness REAL NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS track_bounce_profiles (
          track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
          algorithm_version INTEGER NOT NULL,
          frame_ms INTEGER NOT NULL,
          duration_ms INTEGER NOT NULL,
          file_size INTEGER,
          loudness BLOB NOT NULL,
          onset BLOB NOT NULL,
          created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS lastfm_scrobble_queue (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          attempt_id TEXT NOT NULL UNIQUE,
          lastfm_username TEXT NOT NULL,
          lastfm_account_key TEXT NOT NULL,
          track_id INTEGER,
          artist TEXT NOT NULL,
          track_title TEXT NOT NULL,
          album TEXT,
          duration_seconds INTEGER,
          started_at_utc INTEGER NOT NULL,
          created_at_ms INTEGER NOT NULL,
          attempt_count INTEGER NOT NULL DEFAULT 0,
          next_attempt_at_ms INTEGER,
          last_error_code INTEGER,
          last_error_message TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_lastfm_queue_account_due
          ON lastfm_scrobble_queue (lastfm_account_key, next_attempt_at_ms, started_at_utc, id);
        "#,
    )?;
    ensure_title_source(conn)?;
    Ok(())
}

fn ensure_title_source(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(tracks)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !columns.iter().any(|name| name == "title_source") {
        conn.execute(
            "ALTER TABLE tracks ADD COLUMN title_source TEXT NOT NULL DEFAULT 'filename'",
            [],
        )?;
    }

    let mut stmt =
        conn.prepare("SELECT id, mtproto_document FROM tracks WHERE title_source = 'filename'")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    for (id, document) in rows {
        if document.as_deref().is_some_and(has_structured_audio_title) {
            conn.execute(
                "UPDATE tracks SET title_source = 'telegram_audio' WHERE id = ?1",
                [id],
            )?;
        }
    }
    Ok(())
}

fn has_structured_audio_title(document: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(document) else {
        return false;
    };
    value
        .get("attributes")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|attributes| {
            attributes.iter().any(|attribute| {
                attribute.get("type").and_then(serde_json::Value::as_str)
                    == Some("DocumentAttributeAudio")
                    && attribute
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|title| !title.trim().is_empty())
            })
        })
}
