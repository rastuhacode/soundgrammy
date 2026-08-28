use std::sync::Mutex;

use rusqlite::Connection;

use super::*;

/// Isolated DB per test. File-backed temp paths keyed only by pid+nanos
/// collided under the parallel test harness (`database is locked`).
fn test_db() -> AppResult<Db> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    schema::apply(&conn)?;
    Ok(Db {
        conn: Mutex::new(conn),
    })
}

#[test]
fn load_profile_includes_phone() -> AppResult<()> {
    let db = test_db()?;
    db.save_profile(42, "Ada", Some("Lovelace"), Some("ada"), Some("+1555"))?;
    let profile = db.load_profile()?.expect("profile");
    assert_eq!(profile.tg_user_id, 42);
    assert_eq!(profile.first_name, "Ada");
    assert_eq!(profile.last_name.as_deref(), Some("Lovelace"));
    assert_eq!(profile.username.as_deref(), Some("ada"));
    assert_eq!(profile.phone.as_deref(), Some("+1555"));
    Ok(())
}

#[test]
fn bounce_profile_round_trips_and_cascades_with_track() -> AppResult<()> {
    let db = test_db()?;
    {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tracks
               (id, tg_user_id, file_id, file_unique_id, source, file_size)
             VALUES (7, 42, 'file', 'unique', 'mtproto', 1234)",
            [],
        )?;
    }
    let expected = TrackBounceProfileRecord {
        track_id: 7,
        algorithm_version: 3,
        frame_ms: 50,
        duration_ms: 10_000,
        file_size: Some(1234),
        loudness: vec![1, 2, 3],
        onset: vec![4, 5, 6],
    };
    db.save_track_bounce_profile(&expected)?;
    let loaded = db.track_bounce_profile(7)?.expect("profile");
    assert_eq!(loaded.algorithm_version, expected.algorithm_version);
    assert_eq!(loaded.loudness, expected.loudness);
    assert_eq!(loaded.onset, expected.onset);

    db.conn
        .lock()
        .unwrap()
        .execute("DELETE FROM tracks WHERE id = 7", [])?;
    assert!(db.track_bounce_profile(7)?.is_none());
    Ok(())
}

#[test]
fn clearing_active_profile_keeps_user_scoped_playlists() -> AppResult<()> {
    let db = test_db()?;
    let user_a = 1001;
    let user_c = 2002;

    db.save_profile(user_a, "User A", None, Some("user_a"), None)?;
    let playlist = db.create_playlist(user_a, "Playlist B")?;
    db.clear_active_profile(user_a)?;
    assert!(db.load_profile()?.is_none());

    db.save_profile(user_c, "User C", None, Some("user_c"), None)?;
    let user_c_playlists = db.playlists_bundle(user_c)?;
    assert!(user_c_playlists.custom.is_empty());

    db.clear_active_profile(user_c)?;
    db.save_profile(user_a, "User A", None, Some("user_a"), None)?;
    let user_a_playlists = db.playlists_bundle(user_a)?;

    assert_eq!(user_a_playlists.custom.len(), 1);
    assert_eq!(user_a_playlists.custom[0].id, playlist.id);
    assert_eq!(user_a_playlists.custom[0].name, "Playlist B");
    Ok(())
}

#[test]
fn listen_attempt_updates_aggregates_and_rebuild_matches() -> AppResult<()> {
    use crate::listen_stats::EndReason;

    let db = test_db()?;
    let track_id = 42;

    db.record_attempt_start(track_id)?;
    let end = db.record_attempt_end(track_id, 120_000, Some(180_000), EndReason::Completed)?;
    assert!(end.qualified);
    assert!(!end.early_skip);
    assert_eq!(end.stats.starts, 1);
    assert_eq!(end.stats.completes, 1);
    assert_eq!(end.stats.qualified_plays, 1);
    assert!(end.stats.likeness > 0.0);

    db.record_attempt_start(track_id)?;
    let skip = db.record_attempt_end(track_id, 5_000, Some(180_000), EndReason::Skipped)?;
    assert!(!skip.qualified);
    assert!(skip.early_skip);
    assert_eq!(skip.stats.starts, 2);
    assert_eq!(skip.stats.early_skips, 1);

    let before = db.track_listen_stats(track_id)?.expect("stats row");
    db.rebuild_listen_stats()?;
    let after = db
        .track_listen_stats(track_id)?
        .expect("stats after rebuild");

    assert_eq!(before.starts, after.starts);
    assert_eq!(before.qualified_plays, after.qualified_plays);
    assert_eq!(before.completes, after.completes);
    assert_eq!(before.early_skips, after.early_skips);
    assert_eq!(before.total_listened_ms, after.total_listened_ms);
    Ok(())
}

#[test]
fn listen_events_survive_without_track_row() -> AppResult<()> {
    use crate::listen_stats::EndReason;

    let db = test_db()?;
    // Orphan track id — no FK to tracks.
    db.record_attempt_start(999)?;
    let end = db.record_attempt_end(999, 40_000, Some(60_000), EndReason::Stopped)?;
    assert!(end.qualified);
    assert!(db.track_listen_stats(999)?.is_some());
    Ok(())
}

#[test]
fn clear_listen_stats_removes_events_and_aggregates() -> AppResult<()> {
    use crate::listen_stats::EndReason;

    let db = test_db()?;
    db.record_attempt_start(42)?;
    db.record_attempt_end(42, 60_000, Some(60_000), EndReason::Completed)?;
    assert!(db.track_listen_stats(42)?.is_some());

    db.clear_listen_stats()?;
    assert!(db.all_listen_stats()?.is_empty());

    // Clearing raw events too means a rebuild cannot restore the history.
    db.rebuild_listen_stats()?;
    assert!(db.all_listen_stats()?.is_empty());
    let event_count: i64 =
        db.conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM listen_events", [], |row| row.get(0))?;
    assert_eq!(event_count, 0);
    Ok(())
}

fn upsert_test_track(db: &Db, user: i64, unique: &str, position: i64) -> AppResult<i64> {
    db.upsert_track(&UpsertTrack {
        tg_user_id: user,
        file_id: format!("file-{unique}"),
        file_unique_id: unique.to_string(),
        title: Some(unique.to_string()),
        title_source: "telegram_audio".into(),
        performer: None,
        duration: Some(60),
        mime_type: Some("audio/mpeg".into()),
        file_size: Some(1000),
        track_position: position,
        mtproto_document: "{}".into(),
    })?;
    let tracks = db.tracks_by_user(user)?;
    tracks
        .into_iter()
        .find(|t| t.file_unique_id == unique)
        .map(|t| t.id)
        .ok_or_else(|| crate::error::AppError::msg("track not found after upsert"))
}

#[test]
fn reorder_playlist_tracks_persists_duplicate_order() -> AppResult<()> {
    let db = test_db()?;
    let user = 42;
    db.save_profile(user, "User", None, None, None)?;
    let a = upsert_test_track(&db, user, "dup-a", 0)?;
    let b = upsert_test_track(&db, user, "dup-b", 1)?;
    let playlist = db.create_playlist(user, "Dupes")?;
    db.add_tracks_to_playlist(playlist.id, &[a, b, a], user)?;

    let before = db.playlists_bundle(user)?;
    assert_eq!(before.custom[0].track_ids, vec![a, b, a]);

    db.reorder_playlist_tracks(playlist.id, &[b, a, a], user)?;
    let after = db.playlists_bundle(user)?;
    assert_eq!(after.custom[0].track_ids, vec![b, a, a]);

    db.reorder_playlist_tracks(playlist.id, &[a, a, b], user)?;
    let again = db.playlists_bundle(user)?;
    assert_eq!(again.custom[0].track_ids, vec![a, a, b]);
    Ok(())
}

#[test]
fn cache_settings_default_and_round_trip() -> AppResult<()> {
    let db = test_db()?;

    assert_eq!(
        db.get_setting_i64(SETTING_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES)?,
        DEFAULT_CACHE_LIMIT_BYTES
    );
    assert_eq!(
        db.get_setting_i64(SETTING_CACHE_TTL_SECS, DEFAULT_CACHE_TTL_SECS)?,
        DEFAULT_CACHE_TTL_SECS
    );

    db.set_setting(SETTING_CACHE_LIMIT_BYTES, "1073741824")?;
    db.set_setting(SETTING_CACHE_TTL_SECS, "86400")?;

    assert_eq!(
        db.get_setting_i64(SETTING_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES)?,
        1_073_741_824
    );
    assert_eq!(
        db.get_setting_i64(SETTING_CACHE_TTL_SECS, DEFAULT_CACHE_TTL_SECS)?,
        86_400
    );
    assert_eq!(
        db.get_setting(SETTING_CACHE_LIMIT_BYTES)?.as_deref(),
        Some("1073741824")
    );
    Ok(())
}

#[test]
fn cache_settings_invalid_int_falls_back_to_default() -> AppResult<()> {
    let db = test_db()?;
    db.set_setting(SETTING_CACHE_LIMIT_BYTES, "not-a-number")?;
    assert_eq!(
        db.get_setting_i64(SETTING_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES)?,
        DEFAULT_CACHE_LIMIT_BYTES
    );
    Ok(())
}

#[test]
fn lastfm_queue_is_durable_account_scoped_and_idempotent() -> AppResult<()> {
    let db = test_db()?;
    let row = LastFmQueueInsert {
        attempt_id: "attempt-a".into(),
        username: "Alice".into(),
        account_key: "alice".into(),
        track_id: Some(999),
        artist: "Artist".into(),
        track_title: "Track".into(),
        duration_seconds: Some(31),
        started_at_utc: 100,
        created_at_ms: 1_000,
    };
    assert!(db.enqueue_lastfm_scrobble(&row)?);
    assert!(!db.enqueue_lastfm_scrobble(&row)?);
    assert_eq!(db.lastfm_pending_count("alice")?, 1);
    assert_eq!(db.lastfm_pending_count("bob")?, 0);

    let due = db.due_lastfm_scrobbles("alice", 1_000, 50)?;
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].artist, "Artist");
    db.retry_lastfm_queue_rows(&[due[0].id], 5_000, Some(11), "temporary")?;
    assert!(db.due_lastfm_scrobbles("alice", 4_999, 50)?.is_empty());
    assert_eq!(db.due_lastfm_scrobbles("alice", 5_000, 50)?.len(), 1);

    db.delete_lastfm_queue_for("alice")?;
    assert_eq!(db.lastfm_pending_count("alice")?, 0);
    Ok(())
}

#[test]
fn schema_backfills_only_structured_telegram_audio_titles() -> AppResult<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE tracks (
           id INTEGER PRIMARY KEY, tg_user_id INTEGER NOT NULL, file_id TEXT NOT NULL,
           file_unique_id TEXT NOT NULL UNIQUE, title TEXT, performer TEXT, duration INTEGER,
           source TEXT NOT NULL DEFAULT 'mtproto', mime_type TEXT, file_size INTEGER,
           mtproto_document TEXT, track_position INTEGER, created_at TEXT NOT NULL DEFAULT ''
         );
         INSERT INTO tracks (id, tg_user_id, file_id, file_unique_id, title, mtproto_document)
         VALUES
           (1, 1, '1', '1', 'Structured',
            '{\"attributes\":[{\"type\":\"DocumentAttributeAudio\",\"title\":\"Structured\"}]}'),
           (2, 1, '2', '2', 'filename.mp3',
            '{\"attributes\":[{\"type\":\"DocumentAttributeFilename\",\"fileName\":\"filename.mp3\"}]}'),
           (3, 1, '3', '3', 'broken.mp3', '{broken');",
    )?;
    schema::apply(&conn)?;
    let source = |id| -> rusqlite::Result<String> {
        conn.query_row(
            "SELECT title_source FROM tracks WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
    };
    assert_eq!(source(1)?, "telegram_audio");
    assert_eq!(source(2)?, "filename");
    assert_eq!(source(3)?, "filename");
    Ok(())
}
