use std::sync::Mutex;

use rusqlite::Connection;

use super::*;

/// Isolated DB per test. File-backed temp paths keyed only by pid+nanos
/// collided under the parallel test harness (`database is locked`).
pub(crate) fn test_db() -> AppResult<Db> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    schema::apply(&conn)?;
    Ok(Db {
        conn: Arc::new(Mutex::new(conn)),
    })
}

#[test]
fn library_mode_migration_keeps_initial_progress_hidden_and_completed_cache_visible(
) -> AppResult<()> {
    let db = test_db()?;
    db.save_profile(42, "Test", None, None, None)?;
    upsert_test_track(&db, 42, "1", 0)?;
    {
        let conn = db.conn.lock().unwrap();
        conn.execute_batch("DROP TABLE library_sync_state;
            CREATE TABLE library_sync_state (tg_user_id INTEGER PRIMARY KEY, manifest_hash TEXT NOT NULL,
                next_offset INTEGER NOT NULL DEFAULT 0, complete INTEGER NOT NULL DEFAULT 0,
                verified_at INTEGER, revision INTEGER NOT NULL DEFAULT 0);
            INSERT INTO library_sync_state (tg_user_id, manifest_hash, next_offset) VALUES (42, '1', 1);")?;
        conn.pragma_update(None, "user_version", 0)?;
        schema::apply(&conn)?;
    }
    assert!(db.library_status(42)?.blocking);
    assert_eq!(db.library_status(42)?.processed, 1);
    assert!(db.tracks_page(42, 0, 500, None)?.tracks.is_empty());
    db.conn.lock().unwrap().execute(
        "UPDATE library_sync_state SET complete = 1 WHERE tg_user_id = 42",
        [],
    )?;
    db.mark_synced(42)?;
    assert!(!db.library_status(42)?.blocking);
    assert_eq!(db.tracks_page(42, 0, 500, None)?.tracks.len(), 1);
    Ok(())
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
fn sync_profile_refresh_does_not_restore_a_logged_out_account() -> AppResult<()> {
    let db = test_db()?;
    db.save_profile(42, "Old", None, None, None)?;
    db.refresh_active_profile(42, "New", Some("Name"), Some("new"), None)?;
    assert_eq!(db.load_profile()?.unwrap().first_name, "New");
    db.clear_active_profile(42)?;
    assert!(db
        .refresh_active_profile(42, "Stale", None, None, None)
        .is_err());
    assert!(db.load_profile()?.is_none());
    db.save_profile(99, "Other", None, None, None)?;
    assert!(db
        .refresh_active_profile(42, "Stale", None, None, None)
        .is_err());
    assert_eq!(db.load_profile()?.unwrap().tg_user_id, 99);
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
fn pinned_audio_cache_entries_are_not_downgraded_by_playback_touches() -> AppResult<()> {
    let db = test_db()?;
    let track_id = upsert_test_track(&db, 42, "cache-pinned", 0)?;

    db.mark_audio_cache_pinned(track_id)?;
    db.touch_audio_cache(track_id)?;
    let entries = db.audio_cache_entries()?;
    assert_eq!(entries[&track_id].class, AudioCacheClass::Pinned);

    db.conn
        .lock()
        .unwrap()
        .execute("DELETE FROM tracks WHERE id = ?1", [track_id])?;
    assert!(!db.audio_cache_entries()?.contains_key(&track_id));
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

#[test]
fn create_with_tracks_rolls_back_and_preserves_duplicate_memberships() -> AppResult<()> {
    let db = test_db()?;
    db.conn.lock().unwrap().execute(
        "INSERT INTO tracks (id, tg_user_id, file_id, file_unique_id, source) VALUES (7, 42, 'f', 'u', 'mtproto')", [],
    )?;
    assert!(db
        .create_playlist_with_tracks(42, "Failed", &[7, 999])
        .is_err());
    assert!(db.playlists_bundle(42)?.custom.is_empty());
    let created = db.create_playlist_with_tracks(42, "Saved queue", &[7, 7])?;
    assert_eq!(created.track_ids, vec![7, 7]);
    assert!(db
        .create_playlist_with_tracks(43, "Wrong account", &[7])
        .is_err());
    Ok(())
}

#[test]
fn bulk_removal_rejects_stale_positions_and_rolls_back_invalid_sets() -> AppResult<()> {
    let db = test_db()?;
    db.conn.lock().unwrap().execute(
        "INSERT INTO tracks (id, tg_user_id, file_id, file_unique_id, source) VALUES (7, 42, 'f', 'u', 'mtproto')", [],
    )?;
    let created = db.create_playlist_with_tracks(42, "Queue", &[7, 7, 7])?;
    assert!(db
        .remove_playlist_tracks(created.id, &[0], &[7, 7], 42)
        .is_err());
    assert!(db
        .remove_playlist_tracks(created.id, &[0, 99], &[7, 7, 7], 42)
        .is_err());
    assert_eq!(db.playlists_bundle(42)?.custom[0].track_ids, vec![7, 7, 7]);
    db.remove_playlist_tracks(created.id, &[0, 2], &[7, 7, 7], 42)?;
    assert_eq!(db.playlists_bundle(42)?.custom[0].track_ids, vec![7]);
    Ok(())
}

#[test]
fn bulk_removal_uses_the_same_membership_order_as_the_bundle() -> AppResult<()> {
    let db = test_db()?;
    db.conn.lock().unwrap().execute_batch(
        "INSERT INTO tracks (id, tg_user_id, file_id, file_unique_id, source) VALUES
         (7, 42, 'f7', 'u7', 'mtproto'), (8, 42, 'f8', 'u8', 'mtproto');",
    )?;
    let created = db.create_playlist_with_tracks(42, "Tied positions", &[7, 8])?;
    db.conn.lock().unwrap().execute(
        "UPDATE playlist_tracks SET position = 0, added_at = CASE track_id WHEN 7 THEN '2026-02-01' ELSE '2026-01-01' END WHERE playlist_id = ?1",
        [created.id],
    )?;
    assert_eq!(db.playlists_bundle(42)?.custom[0].track_ids, vec![8, 7]);
    db.remove_playlist_tracks(created.id, &[0], &[8, 7], 42)?;
    assert_eq!(db.playlists_bundle(42)?.custom[0].track_ids, vec![7]);
    Ok(())
}

#[test]
fn accounts_can_index_the_same_document_without_changing_local_ids() -> AppResult<()> {
    let db = test_db()?;
    let a = upsert_test_track(&db, 1, "shared", 0)?;
    let playlist = db.create_playlist_with_tracks(1, "A", &[a, a])?;
    let b = upsert_test_track(&db, 2, "shared", 0)?;
    assert_ne!(a, b);
    assert_eq!(upsert_test_track(&db, 1, "shared", 1)?, a);
    assert_eq!(db.playlists_bundle(1)?.custom[0].id, playlist.id);
    assert_eq!(db.playlists_bundle(1)?.custom[0].track_ids, vec![a, a]);
    assert!(db.track_by_id(a, 2)?.is_none());
    Ok(())
}

#[test]
fn append_and_like_reject_foreign_tracks_without_partial_changes() -> AppResult<()> {
    let db = test_db()?;
    let a = upsert_test_track(&db, 1, "a", 0)?;
    let b = upsert_test_track(&db, 2, "b", 0)?;
    let playlist = db.create_playlist(2, "B")?;
    assert!(db.add_track_to_playlist(playlist.id, a, 2).is_err());
    assert!(db.add_tracks_to_playlist(playlist.id, &[b, a], 2).is_err());
    assert!(db.toggle_like(a, 2).is_err());
    let bundle = db.playlists_bundle(2)?;
    assert!(bundle.custom[0].track_ids.is_empty());
    assert!(bundle.liked.track_ids.is_empty());
    Ok(())
}

#[test]
fn selecting_a_profile_replaces_the_active_identity() -> AppResult<()> {
    let db = test_db()?;
    db.save_profile(1, "A", None, None, None)?;
    db.save_profile(2, "B", None, None, None)?;
    assert_eq!(db.load_profile()?.unwrap().tg_user_id, 2);
    assert!(db
        .refresh_active_profile(1, "Late", None, None, None)
        .is_err());
    assert_eq!(db.load_profile()?.unwrap().first_name, "B");
    Ok(())
}

#[test]
fn stale_refresh_is_rejected_even_after_same_account_logs_in_again() -> AppResult<()> {
    let db = test_db()?;
    db.save_profile(1, "Original", None, None, None)?;
    let generation = db.account_generation()?;
    db.clear_active_profile(1)?;
    db.save_profile(1, "New login", None, None, None)?;
    assert!(db
        .refresh_profile_at_generation(1, generation, "Late", None, None, None)
        .is_err());
    assert_eq!(db.load_profile()?.unwrap().first_name, "New login");
    Ok(())
}

#[test]
fn account_identity_migration_preserves_memberships_pins_and_constraints() -> AppResult<()> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch("CREATE TABLE tracks (
        id INTEGER PRIMARY KEY AUTOINCREMENT, tg_user_id INTEGER NOT NULL, file_id TEXT NOT NULL,
        file_unique_id TEXT NOT NULL UNIQUE,title TEXT,performer TEXT,duration INTEGER,
        source TEXT NOT NULL DEFAULT 'mtproto',mime_type TEXT,file_size INTEGER,mtproto_document TEXT,
        track_position INTEGER,created_at TEXT NOT NULL DEFAULT '');
        INSERT INTO tracks(id,tg_user_id,file_id,file_unique_id) VALUES(9,1,'shared','shared');
        CREATE TABLE playlists(id INTEGER PRIMARY KEY AUTOINCREMENT,tg_user_id INTEGER NOT NULL,
            name TEXT NOT NULL,kind TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT '',updated_at TEXT NOT NULL DEFAULT '');
        INSERT INTO playlists(id,tg_user_id,name,kind) VALUES(7,1,'Old','custom');
        CREATE TABLE playlist_tracks(id INTEGER PRIMARY KEY AUTOINCREMENT,
            playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            position INTEGER NOT NULL DEFAULT 0,added_at TEXT NOT NULL DEFAULT '');
        INSERT INTO playlist_tracks(id,playlist_id,track_id,position) VALUES(5,7,9,0);
        CREATE TABLE audio_cache_entries(track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
            cache_class TEXT NOT NULL DEFAULT 'automatic',last_accessed_at_ms INTEGER NOT NULL);
        INSERT INTO audio_cache_entries VALUES(9,'pinned',1);")?;
    schema::apply(&conn)?;
    schema::apply(&conn)?;
    let db = Db {
        conn: Arc::new(Mutex::new(conn)),
    };
    assert_eq!(db.playlists_bundle(1)?.custom[0].track_ids, vec![9]);
    assert_eq!(db.audio_cache_entries()?[&9].class, AudioCacheClass::Pinned);
    assert_ne!(upsert_test_track(&db, 2, "shared", 0)?, 9);
    let conn = db.conn.lock().unwrap();
    assert!(conn
        .execute("UPDATE tracks SET title_source='invalid' WHERE id=9", [])
        .is_err());
    conn.execute("DELETE FROM tracks WHERE id=9", [])?;
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM playlist_tracks", [], |r| r
            .get::<_, i64>(0))?,
        0
    );
    assert!(conn.pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))?);
    Ok(())
}

#[test]
fn history_is_account_scoped_and_survives_remove_then_resave() -> AppResult<()> {
    use crate::listen_stats::EndReason;
    let db = test_db()?;
    let a = upsert_test_track(&db, 1, "song", 0)?;
    let b = upsert_test_track(&db, 2, "song", 0)?;
    db.record_attempt_end(a, 60_000, Some(60_000), EndReason::Completed)?;
    db.record_attempt_end(b, 60_000, Some(60_000), EndReason::Completed)?;
    db.conn
        .lock()
        .unwrap()
        .execute("DELETE FROM tracks WHERE id=?1", [a])?;
    assert_eq!(upsert_test_track(&db, 1, "song", 0)?, a);
    assert_eq!(db.listen_stats_for_user(1)?.len(), 1);
    db.clear_listen_stats_for_user(1)?;
    assert!(db.listen_stats_for_user(1)?.is_empty());
    assert_eq!(db.listen_stats_for_user(2)?.len(), 1);
    db.rebuild_listen_stats()?;
    assert!(db.listen_stats_for_user(1)?.is_empty());
    assert_eq!(db.listen_stats_for_user(2)?[0].qualified_plays, 1);
    Ok(())
}

#[test]
fn reorder_checks_the_viewed_order_and_preserves_duplicate_membership_ids() -> AppResult<()> {
    let db = test_db()?;
    let a = upsert_test_track(&db, 1, "a", 0)?;
    let b = upsert_test_track(&db, 1, "b", 1)?;
    let playlist = db.create_playlist_with_tracks(1, "List", &[a, a, b])?;
    let memberships = |db: &Db| -> AppResult<Vec<i64>> {
        let conn = db.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id FROM playlist_tracks WHERE playlist_id=?1 ORDER BY position")?;
        let rows = stmt
            .query_map([playlist.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    };
    let before = memberships(&db)?;
    db.reorder_playlist_tracks_guarded(playlist.id, &[a, b, a], &[a, a, b], Some([1, 2]), 1)?;
    assert_eq!(memberships(&db)?, vec![before[0], before[2], before[1]]);
    assert!(db
        .reorder_playlist_tracks_guarded(playlist.id, &[b, a, a], &[a, a, b], None, 1)
        .is_err());
    let unicode = "я".repeat(100);
    db.update_playlist(playlist.id, 1, Some(&unicode))?;
    assert!(db
        .update_playlist(playlist.id, 1, Some(&"я".repeat(101)))
        .is_err());
    Ok(())
}

#[test]
fn migrations_do_not_reuse_unknown_historical_track_ids() -> AppResult<()> {
    let db = test_db()?;
    db.record_attempt_end(
        9000,
        10_000,
        Some(60_000),
        crate::listen_stats::EndReason::Stopped,
    )?;
    {
        let conn = db.conn.lock().unwrap();
        conn.pragma_update(None, "user_version", 1)?;
        schema::apply(&conn)?;
    }
    assert!(upsert_test_track(&db, 1, "new", 0)? > 9000);
    Ok(())
}

#[test]
fn interrupted_migration_rolls_back_and_can_be_retried() -> AppResult<()> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", false)?;
    conn.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY AUTOINCREMENT,tg_user_id INTEGER NOT NULL,file_id TEXT NOT NULL,file_unique_id TEXT NOT NULL UNIQUE,title TEXT,performer TEXT,duration INTEGER,source TEXT NOT NULL DEFAULT 'mtproto',mime_type TEXT,file_size INTEGER,mtproto_document TEXT,track_position INTEGER,created_at TEXT NOT NULL DEFAULT '');
        INSERT INTO tracks(id,tg_user_id,file_id,file_unique_id) VALUES(7,1,'7','7');
        CREATE TABLE playlists(id INTEGER PRIMARY KEY AUTOINCREMENT,tg_user_id INTEGER NOT NULL,name TEXT NOT NULL,kind TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT '',updated_at TEXT NOT NULL DEFAULT '');
        INSERT INTO playlists(id,tg_user_id,name,kind) VALUES(1,1,'Old','custom');
        CREATE TABLE playlist_tracks(id INTEGER PRIMARY KEY AUTOINCREMENT,playlist_id INTEGER NOT NULL REFERENCES playlists(id),track_id INTEGER NOT NULL REFERENCES tracks(id),position INTEGER NOT NULL DEFAULT 0,added_at TEXT NOT NULL DEFAULT '');
        INSERT INTO playlist_tracks(playlist_id,track_id) VALUES(1,999);")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    assert!(schema::apply(&conn).is_err());
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))?,
        0
    );
    assert!(conn.pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))?);
    assert_eq!(
        conn.query_row("SELECT file_unique_id FROM tracks WHERE id=7", [], |r| {
            r.get::<_, String>(0)
        })?,
        "7"
    );
    conn.execute("DELETE FROM playlist_tracks", [])?;
    schema::apply(&conn)?;
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))?,
        2
    );
    Ok(())
}

#[test]
fn listen_rebuild_cannot_restore_a_concurrent_clear_or_overwrite_new_events() -> AppResult<()> {
    use crate::listen_stats::EndReason;
    for clear in [true, false] {
        let db = test_db()?;
        let track = upsert_test_track(&db, 1, "123", 0)?;
        db.record_attempt_end(track, 60000, Some(120000), EndReason::Completed)?;
        assert!(db
            .rebuild_listen_stats_with(|| {
                if clear {
                    db.clear_listen_stats_for_user(1).unwrap();
                } else {
                    db.record_attempt_end(track, 60000, Some(120000), EndReason::Completed)
                        .unwrap();
                }
            })
            .is_err());
        if clear {
            assert!(db.listen_stats_for_user(1)?.is_empty());
        } else {
            assert_eq!(db.track_listen_stats(track)?.unwrap().starts, 2);
        }
    }
    Ok(())
}

#[test]
fn legacy_ambiguous_profiles_require_login_and_foreign_memberships_are_repaired() -> AppResult<()> {
    let db = test_db()?;
    let owned = upsert_test_track(&db, 1, "123", 0)?;
    let foreign = upsert_test_track(&db, 2, "124", 0)?;
    let playlist = db.create_playlist_with_tracks(1, "List", &[owned])?;
    let conn = db.conn.lock().unwrap();
    conn.execute_batch(
        "DROP TRIGGER profile_single_insert; DROP TRIGGER playlist_track_owner_insert;
        INSERT INTO profile(tg_user_id,first_name) VALUES(1,'A'),(2,'B');",
    )?;
    conn.execute(
        "INSERT INTO playlist_tracks(playlist_id,track_id,position) VALUES(?1,?2,1)",
        rusqlite::params![playlist.id, foreign],
    )?;
    conn.pragma_update(None, "user_version", 0)?;
    schema::apply(&conn)?;
    drop(conn);
    assert!(db.load_profile()?.is_none());
    assert!(db.track_by_id(owned, 1)?.is_some() && db.track_by_id(foreign, 2)?.is_some());
    assert_eq!(db.playlists_bundle(1)?.custom[0].track_ids, vec![owned]);
    Ok(())
}
