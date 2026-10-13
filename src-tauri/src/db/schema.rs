use rusqlite::{Connection, OptionalExtension};

use crate::error::AppResult;

pub(super) fn apply(conn: &Connection) -> AppResult<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > 2 {
        return Err(crate::error::AppError::msg(
            "Database was created by a newer SoundGrammy version",
        ));
    }
    if version == 2 {
        return Ok(());
    }
    let foreign_keys: bool = conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = (|| {
        if version == 0 {
            migrate(conn)?;
        }
        migrate_listen_identity(conn)
    })();
    conn.pragma_update(None, "foreign_keys", foreign_keys)?;
    result
}

fn migrate(conn: &Connection) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    let conn = &tx;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tracks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          tg_user_id INTEGER NOT NULL,
          file_id TEXT NOT NULL,
          file_unique_id TEXT NOT NULL,
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
          created_at TEXT NOT NULL DEFAULT (datetime('now')),
          UNIQUE (tg_user_id, file_unique_id)
        );

        CREATE INDEX IF NOT EXISTS idx_tracks_tg_user_id ON tracks (tg_user_id);
        CREATE INDEX IF NOT EXISTS idx_tracks_user_position
          ON tracks (tg_user_id, track_position, created_at DESC, id DESC);

        -- Separate from the legacy XOR hash. A manifest can be current while
        -- metadata/order indexing is incomplete after a failed request.
        CREATE TABLE IF NOT EXISTS library_sync_state (
          tg_user_id INTEGER PRIMARY KEY,
          manifest_hash TEXT NOT NULL,
          next_offset INTEGER NOT NULL DEFAULT 0,
          complete INTEGER NOT NULL DEFAULT 0,
          blocking INTEGER NOT NULL DEFAULT 0,
          verified_at INTEGER,
          revision INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS library_sync_manifest (
          tg_user_id INTEGER NOT NULL,
          document_id TEXT NOT NULL,
          ordinal INTEGER NOT NULL,
          PRIMARY KEY (tg_user_id, document_id)
        );
        CREATE TABLE IF NOT EXISTS library_sync_entries (
          tg_user_id INTEGER NOT NULL,
          document_id TEXT NOT NULL,
          position INTEGER NOT NULL,
          PRIMARY KEY (tg_user_id, document_id),
          UNIQUE (tg_user_id, position)
        );
        CREATE TABLE IF NOT EXISTS library_sync_backoff (
          tg_user_id INTEGER PRIMARY KEY,
          retry_at INTEGER NOT NULL
        );

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

        CREATE TABLE IF NOT EXISTS audio_cache_entries (
          track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
          cache_class TEXT NOT NULL DEFAULT 'automatic'
            CHECK (cache_class IN ('automatic', 'pinned')),
          last_accessed_at_ms INTEGER NOT NULL
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
    ensure_library_sync_mode(conn)?;
    migrate_track_identity(conn)?;
    // Repair relationships previously accepted by permissive append/like paths.
    // Keep tracks/history intact; a foreign-account track is not a valid membership.
    conn.execute_batch(
        "DELETE FROM playlist_tracks WHERE EXISTS(
        SELECT 1 FROM playlists p JOIN tracks t ON t.id=playlist_tracks.track_id
        WHERE p.id=playlist_tracks.playlist_id AND p.tg_user_id != t.tg_user_id);",
    )?;
    // Legacy application state was intended to hold exactly one active profile.
    conn.execute_batch("DELETE FROM profile WHERE (SELECT COUNT(*) FROM profile)>1;
        CREATE TRIGGER IF NOT EXISTS profile_single_insert BEFORE INSERT ON profile
        WHEN EXISTS(SELECT 1 FROM profile WHERE tg_user_id != NEW.tg_user_id)
        BEGIN SELECT RAISE(ABORT, 'Only one active profile is allowed'); END;
        CREATE TRIGGER IF NOT EXISTS playlist_track_owner_insert BEFORE INSERT ON playlist_tracks
        WHEN NOT EXISTS(SELECT 1 FROM tracks t JOIN playlists p ON p.tg_user_id=t.tg_user_id
            WHERE t.id=NEW.track_id AND p.id=NEW.playlist_id)
        BEGIN SELECT RAISE(ABORT, 'Track belongs to another account'); END;
        CREATE TRIGGER IF NOT EXISTS playlist_track_owner_update BEFORE UPDATE OF track_id,playlist_id ON playlist_tracks
        WHEN NOT EXISTS(SELECT 1 FROM tracks t JOIN playlists p ON p.tg_user_id=t.tg_user_id
            WHERE t.id=NEW.track_id AND p.id=NEW.playlist_id)
        BEGIN SELECT RAISE(ABORT, 'Track belongs to another account'); END;")?;
    let violations: i64 =
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if violations != 0 {
        return Err(crate::error::AppError::msg(
            "Database migration found invalid foreign keys",
        ));
    }
    conn.pragma_update(None, "user_version", 1)?;
    tx.commit()?;
    Ok(())
}

fn migrate_listen_identity(conn: &Connection) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS listen_track_identity (
        track_id INTEGER PRIMARY KEY, tg_user_id INTEGER, file_unique_id TEXT,
        UNIQUE(tg_user_id,file_unique_id));
        INSERT OR IGNORE INTO listen_track_identity SELECT id,tg_user_id,file_unique_id FROM tracks;
        INSERT OR IGNORE INTO listen_track_identity(track_id) SELECT DISTINCT track_id FROM listen_events;
        CREATE INDEX IF NOT EXISTS idx_listen_identity_user ON listen_track_identity(tg_user_id);")?;
    let highest: i64 = tx.query_row(
        "SELECT COALESCE(MAX(track_id),0) FROM listen_track_identity",
        [],
        |r| r.get(0),
    )?;
    if tx.execute(
        "UPDATE sqlite_sequence SET seq=MAX(seq,?1) WHERE name='tracks'",
        [highest],
    )? == 0
    {
        tx.execute(
            "INSERT INTO sqlite_sequence(name,seq) VALUES('tracks',?1)",
            [highest],
        )?;
    }
    tx.pragma_update(None, "user_version", 2)?;
    tx.commit()?;
    Ok(())
}

fn migrate_track_identity(conn: &Connection) -> AppResult<()> {
    let global_unique: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_index_list('tracks') i
         WHERE i.\"unique\"=1 AND (SELECT group_concat(name) FROM pragma_index_info(i.name))='file_unique_id')",
        [], |row| row.get(0))?;
    if !global_unique {
        return Ok(());
    }
    let previous_sequence: i64 = conn
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='tracks'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    conn.execute_batch("CREATE TABLE tracks_next (
        id INTEGER PRIMARY KEY AUTOINCREMENT, tg_user_id INTEGER NOT NULL,
        file_id TEXT NOT NULL, file_unique_id TEXT NOT NULL, title TEXT,
        title_source TEXT NOT NULL DEFAULT 'filename' CHECK(title_source IN ('telegram_audio','filename','user_override')),
        performer TEXT, duration INTEGER, source TEXT NOT NULL DEFAULT 'mtproto', mime_type TEXT,
        file_size INTEGER, mtproto_document TEXT, track_position INTEGER,
        created_at TEXT NOT NULL DEFAULT (datetime('now')), UNIQUE(tg_user_id,file_unique_id));
        INSERT INTO tracks_next SELECT id,tg_user_id,file_id,file_unique_id,title,
          CASE WHEN title_source IN ('telegram_audio','filename','user_override') THEN title_source ELSE 'filename' END,
          performer,duration,source,mime_type,file_size,mtproto_document,track_position,created_at FROM tracks;
        DROP TABLE tracks;
        ALTER TABLE tracks_next RENAME TO tracks;
        CREATE INDEX idx_tracks_tg_user_id ON tracks(tg_user_id);
        CREATE INDEX idx_tracks_user_position ON tracks(tg_user_id,track_position,created_at DESC,id DESC);")?;
    conn.execute(
        "UPDATE sqlite_sequence SET seq=MAX(seq,?1) WHERE name='tracks'",
        [previous_sequence],
    )?;
    Ok(())
}

fn ensure_library_sync_mode(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(library_sync_state)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !columns.iter().any(|column| column == "blocking") {
        conn.execute(
            "ALTER TABLE library_sync_state ADD COLUMN blocking INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
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
