use std::sync::Mutex;
use std::time::Instant;

use super::*;
use crate::db::test_db;

const UID: i64 = 42;

fn track(id: i64, position: i64) -> UpsertTrack {
    UpsertTrack {
        tg_user_id: UID,
        file_id: id.to_string(),
        file_unique_id: id.to_string(),
        title: Some(format!("Track {id}")),
        title_source: "telegram_audio".into(),
        performer: Some("Artist".into()),
        duration: Some(120),
        mime_type: Some("audio/mpeg".into()),
        file_size: Some(1000),
        track_position: position,
        mtproto_document: "{\"accessHash\":\"backend-only\"}".into(),
    }
}

struct Source {
    ids: Vec<i64>,
    page_ids: Vec<i64>,
    fail_at: Option<i32>,
    fail_manifest_call: Option<usize>,
    final_ids: Option<Vec<i64>>,
    page_size: usize,
    calls: Mutex<Vec<i32>>,
    hashes: Mutex<Vec<i64>>,
}

impl Source {
    fn new(ids: Vec<i64>) -> Self {
        Self {
            page_ids: ids.clone(),
            ids,
            fail_at: None,
            fail_manifest_call: None,
            final_ids: None,
            page_size: PAGE_LIMIT as usize,
            calls: Mutex::new(vec![]),
            hashes: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl MusicSource for Source {
    async fn manifest(&self, hash: i64) -> AppResult<Option<Vec<i64>>> {
        let mut calls = self.hashes.lock().unwrap();
        if self.fail_manifest_call == Some(calls.len() + 1) {
            calls.push(hash);
            return Err(AppError::msg(
                "network unavailable during final verification",
            ));
        }
        let final_call = !calls.is_empty() && !self.calls.lock().unwrap().is_empty();
        calls.push(hash);
        let ids = if final_call {
            self.final_ids.as_ref().unwrap_or(&self.ids)
        } else {
            &self.ids
        };
        Ok((hash != compute_saved_music_hash(ids)).then(|| ids.clone()))
    }
    async fn page(&self, offset: i32) -> AppResult<MusicPage> {
        self.calls.lock().unwrap().push(offset);
        if self.fail_at == Some(offset) {
            return Err(AppError::msg("network unavailable"));
        }
        Ok(MusicPage {
            total: self.ids.len() as i64,
            tracks: self
                .page_ids
                .iter()
                .skip(offset as usize)
                .take(self.page_size)
                .enumerate()
                .map(|(index, id)| track(*id, i64::from(offset) + index as i64))
                .collect(),
        })
    }
}

fn active_db() -> AppResult<Db> {
    let db = test_db()?;
    db.save_profile(UID, "Test", None, None, None)?;
    Ok(db)
}

#[tokio::test]
async fn large_initial_index_is_durable_but_hidden_until_complete() -> AppResult<()> {
    let db = active_db()?;
    let source = Source::new((1..=10_037).collect());
    let mut early_visible = false;
    let result = index(&db, UID, &source, |batch, done| {
        assert!(batch.tracks.is_empty());
        assert!(!serde_json::to_string(&batch)
            .unwrap()
            .contains("backend-only"));
        if done == 100 {
            // The first page is readable before the remaining 99+ requests run.
            assert_eq!(source.calls.lock().unwrap().as_slice(), &[0]);
            assert_eq!(db.tracks_by_user(UID).unwrap().len(), 100);
            assert!(db.tracks_page(UID, 0, 500, None).unwrap().tracks.is_empty());
            assert!(batch.status.blocking);
            early_visible = true;
        }
    })
    .await?;
    assert!(result.changed && early_visible);
    assert_eq!(result.total, 10_037);
    assert_eq!(source.calls.lock().unwrap().len(), 101);
    assert!(db.library_status(UID)?.complete);

    let started = Instant::now();
    let mut offset = 0;
    let mut revision = None;
    let mut ids = vec![];
    let mut pages = 0;
    loop {
        let page = db.tracks_page(UID, offset, 500, revision)?;
        revision = Some(page.status.revision);
        assert!(page.tracks.len() <= 500);
        assert!(page
            .tracks
            .iter()
            .all(|t| t.track.mtproto_document.is_none()));
        ids.extend(
            page.tracks
                .into_iter()
                .map(|t| t.track.file_unique_id.parse::<i64>().unwrap()),
        );
        pages += 1;
        let Some(next) = page.next_offset else {
            break;
        };
        offset = next;
    }
    eprintln!(
        "10,037 metadata rows: {pages} local pages in {:?}",
        started.elapsed()
    );
    assert_eq!(ids, source.ids);
    Ok(())
}

#[tokio::test]
async fn complete_unchanged_manifest_skips_all_metadata_requests() -> AppResult<()> {
    let db = active_db()?;
    let source = Source::new((1..=203).collect());
    index(&db, UID, &source, |_, _| {}).await?;
    let unchanged = Source::new(source.ids.clone());
    let result = index(&db, UID, &unchanged, |_, _| {
        panic!("unchanged scan must not publish pages")
    })
    .await?;
    assert!(!result.changed);
    assert_eq!(
        unchanged.hashes.lock().unwrap().as_slice(),
        &[compute_saved_music_hash(&source.ids)]
    );
    assert!(unchanged.calls.lock().unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn confirmed_full_reindex_refetches_unchanged_metadata_and_order() -> AppResult<()> {
    let db = active_db()?;
    let source = Source::new((1..=103).collect());
    index(&db, UID, &source, |_, _| {}).await?;
    db.request_full_reindex(UID)?;
    assert!(db.library_status(UID)?.blocking);
    assert!(db.tracks_page(UID, 0, 500, None)?.tracks.is_empty());
    let mut reordered = Source::new(source.ids.clone());
    reordered.page_ids.swap(101, 102);
    assert!(
        index(&db, UID, &reordered, |batch, _| assert!(batch
            .tracks
            .is_empty()))
        .await?
        .changed
    );
    assert_eq!(reordered.calls.lock().unwrap().as_slice(), &[0, 100]);
    assert_eq!(db.tracks_by_user(UID)?[101].file_unique_id, "103");
    Ok(())
}

#[tokio::test]
async fn failed_scan_keeps_pages_and_playlists_then_resumes_with_overlap() -> AppResult<()> {
    let db = active_db()?;
    db.upsert_track(&track(9000, 0))?;
    let old = db.tracks_by_user(UID)?[0].id;
    let playlist = db.create_playlist_with_tracks(UID, "Keep until verified", &[old, old])?;
    let mut failing = Source::new((1..=203).collect());
    failing.fail_at = Some(100);
    assert!(index(&db, UID, &failing, |_, _| {}).await.is_err());
    assert_eq!(db.tracks_by_user(UID)?.len(), 101);
    assert_eq!(
        db.playlists_bundle(UID)?.custom[0].track_ids,
        vec![old, old]
    );
    let checkpoint = db.saved_music_checkpoint(UID)?.unwrap();
    assert_eq!(checkpoint.next_offset, 100);
    assert!(!checkpoint.complete);
    assert!(db.last_sync_at(UID)?.is_none());

    let resumed = Source::new(failing.ids.clone());
    let mut emitted = vec![];
    index(&db, UID, &resumed, |batch, _| {
        emitted.extend(batch.tracks.iter().map(|t| t.track.file_unique_id.clone()));
    })
    .await?;
    assert_eq!(resumed.calls.lock().unwrap().as_slice(), &[0, 100, 200]);
    assert!(emitted.is_empty()); // full indexing never publishes partial tracks
    assert_eq!(db.tracks_by_user(UID)?.len(), 203);
    let bundle = db.playlists_bundle(UID)?;
    assert_eq!(bundle.custom[0].id, playlist.id);
    assert!(bundle.custom[0].track_ids.is_empty());
    Ok(())
}

#[tokio::test]
async fn failed_final_verification_resumes_without_refetching_the_catalogue() -> AppResult<()> {
    let db = active_db()?;
    db.upsert_track(&track(9000, 0))?;
    let mut failing = Source::new((1..=203).collect());
    failing.fail_manifest_call = Some(2);
    assert!(index(&db, UID, &failing, |_, _| {}).await.is_err());
    assert_eq!(db.tracks_by_user(UID)?.len(), 204);
    let checkpoint = db.saved_music_checkpoint(UID)?.unwrap();
    assert_eq!(checkpoint.next_offset, 203);
    assert!(!checkpoint.complete);
    let resumed = Source::new(failing.ids.clone());
    index(&db, UID, &resumed, |_, _| {}).await?;
    assert_eq!(resumed.calls.lock().unwrap().as_slice(), &[103]);
    assert_eq!(db.tracks_by_user(UID)?.len(), 203);
    Ok(())
}

#[tokio::test]
async fn checkpoint_and_partial_tracks_survive_database_reopen() -> AppResult<()> {
    let path = std::env::temp_dir().join(format!(
        "soundgrammy-library-resume-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    {
        let db = Db::open(&path)?;
        db.save_profile(UID, "Test", None, None, None)?;
        let mut source = Source::new((1..=143).collect());
        source.fail_at = Some(100);
        assert!(index(&db, UID, &source, |_, _| {}).await.is_err());
        db.defer_saved_music_sync(UID, 1600)?;
    }
    {
        let db = Db::open(&path)?;
        assert_eq!(db.tracks_by_user(UID)?.len(), 100);
        assert_eq!(db.saved_music_checkpoint(UID)?.unwrap().next_offset, 100);
        assert_eq!(db.saved_music_wait_seconds(UID, 1000)?, 600);
        let resumed = Source::new((1..=143).collect());
        index(&db, UID, &resumed, |_, _| {}).await?;
        assert_eq!(resumed.calls.lock().unwrap().as_slice(), &[0, 100]);
        assert!(db.library_status(UID)?.complete);
    }
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn flood_wait_deadlines_are_account_scoped_and_cannot_be_shortened() -> AppResult<()> {
    let db = active_db()?;
    db.defer_saved_music_sync(UID, 1600)?;
    db.defer_saved_music_sync(UID, 1100)?;
    assert_eq!(db.saved_music_wait_seconds(UID, 1000)?, 600);
    assert_eq!(db.saved_music_wait_seconds(99, 1000)?, 0);
    assert_eq!(db.saved_music_wait_seconds(UID, 1600)?, 0);
    assert_eq!(db.saved_music_wait_seconds(UID, 1700)?, 0);
    db.clear_active_profile(UID)?;
    assert!(db.defer_saved_music_sync(UID, 2000).is_err());
    Ok(())
}

#[test]
fn recognizes_long_and_premium_flood_waits_without_throttling_other_errors() {
    let rpc = |name: &str, value| {
        ferogram::InvocationError::Rpc(ferogram::RpcError {
            code: 420,
            name: name.into(),
            value,
        })
    };
    assert_eq!(flood_wait_seconds(&rpc("FLOOD_WAIT", Some(600))), Some(600));
    assert_eq!(
        flood_wait_seconds(&rpc("FLOOD_PREMIUM_WAIT", Some(90))),
        Some(90)
    );
    assert_eq!(flood_wait_seconds(&rpc("FLOOD_WAIT", Some(0))), None);
    assert_eq!(flood_wait_seconds(&rpc("FLOOD_WAIT", None)), None);
    assert_eq!(flood_wait_seconds(&rpc("SLOWMODE_WAIT", Some(90))), None);
}

#[tokio::test]
async fn remote_reordering_preserves_local_ids_and_duplicate_playlist_memberships() -> AppResult<()>
{
    let db = active_db()?;
    index(&db, UID, &Source::new(vec![1, 2, 3]), |_, _| {}).await?;
    let before = db.tracks_by_user(UID)?;
    let first = before[0].id;
    let third = before[2].id;
    db.create_playlist_with_tracks(UID, "Duplicates", &[first, third, first])?;
    index(&db, UID, &Source::new(vec![3, 1, 2]), |_, _| {}).await?;
    let after = db.tracks_by_user(UID)?;
    assert_eq!(after[0].id, third);
    assert_eq!(after[1].id, first);
    assert_eq!(
        db.playlists_bundle(UID)?.custom[0].track_ids,
        vec![first, third, first]
    );
    Ok(())
}

#[tokio::test]
async fn legacy_cached_library_gets_verified_without_replacing_track_ids() -> AppResult<()> {
    let db = active_db()?;
    db.upsert_track(&track(1, 0))?;
    db.mark_synced(UID)?;
    let old = db.tracks_by_user(UID)?[0].id;
    assert!(db.saved_music_checkpoint(UID)?.is_none());
    let source = Source::new(vec![1]);
    assert!(index(&db, UID, &source, |_, _| {}).await?.changed);
    assert_eq!(source.hashes.lock().unwrap()[0], 0);
    assert_eq!(source.calls.lock().unwrap().as_slice(), &[0]);
    assert_eq!(db.tracks_by_user(UID)?[0].id, old);
    Ok(())
}

#[tokio::test]
async fn changed_resume_boundary_restarts_order_scan_without_losing_metadata() -> AppResult<()> {
    let db = active_db()?;
    let mut failing = Source::new((1..=203).collect());
    failing.fail_at = Some(100);
    assert!(index(&db, UID, &failing, |_, _| {}).await.is_err());
    let mut reordered = Source::new(failing.ids.clone());
    reordered.page_ids.swap(0, 1);
    index(&db, UID, &reordered, |_, _| {}).await?;
    assert_eq!(
        reordered.calls.lock().unwrap().as_slice(),
        &[0, 0, 100, 200]
    );
    assert_eq!(db.tracks_by_user(UID)?[0].file_unique_id, "2");
    Ok(())
}

#[tokio::test]
async fn same_count_replacement_is_not_mistaken_for_unchanged_library() -> AppResult<()> {
    let db = active_db()?;
    index(&db, UID, &Source::new((1..=201).collect()), |_, _| {}).await?;
    let replaced = Source::new((2..=202).collect());
    assert!(index(&db, UID, &replaced, |_, _| {}).await?.changed);
    let tracks = db.tracks_by_user(UID)?;
    assert_eq!(tracks.len(), 201);
    assert_eq!(tracks[0].file_unique_id, "2");
    assert_eq!(tracks.last().unwrap().file_unique_id, "202");
    Ok(())
}

#[tokio::test]
async fn final_manifest_change_does_not_delete_any_old_membership() -> AppResult<()> {
    let db = active_db()?;
    db.upsert_track(&track(9000, 0))?;
    let old = db.tracks_by_user(UID)?[0].id;
    db.create_playlist_with_tracks(UID, "Old", &[old])?;
    let mut changing = Source::new((1..=103).collect());
    changing.final_ids = Some((2..=104).collect());
    assert!(index(&db, UID, &changing, |_, _| {}).await.is_err());
    assert!(db.track_by_id(old, UID)?.is_some());
    assert_eq!(db.playlists_bundle(UID)?.custom[0].track_ids, vec![old]);
    assert_eq!(db.saved_music_checkpoint(UID)?.unwrap().next_offset, 0);
    assert!(db.last_sync_at(UID)?.is_none());
    Ok(())
}

#[tokio::test]
async fn short_nonempty_pages_continue_until_all_manifest_ids_are_indexed() -> AppResult<()> {
    let db = active_db()?;
    let mut source = Source::new((1..=19).collect());
    source.page_size = 7;
    index(&db, UID, &source, |_, _| {}).await?;
    assert_eq!(source.calls.lock().unwrap().as_slice(), &[0, 7, 14]);
    assert_eq!(db.tracks_by_user(UID)?.len(), 19);
    Ok(())
}

#[tokio::test]
async fn premature_empty_and_duplicate_pages_cannot_prune_tracks() -> AppResult<()> {
    for duplicate in [false, true] {
        let db = active_db()?;
        db.upsert_track(&track(9000, 0))?;
        let mut source = Source::new((1..=103).collect());
        if duplicate {
            source.page_ids[100] = 1;
        } else {
            source.page_ids.truncate(100);
        }
        assert!(index(&db, UID, &source, |_, _| {}).await.is_err());
        assert_eq!(db.tracks_by_user(UID)?.len(), 101);
        assert_eq!(db.saved_music_checkpoint(UID)?.unwrap().next_offset, 0);
        assert!(!db.library_status(UID)?.complete);
    }
    Ok(())
}

#[tokio::test]
async fn verified_empty_manifest_clears_library_and_can_be_cached() -> AppResult<()> {
    let db = active_db()?;
    db.upsert_track(&track(1, 0))?;
    let empty = Source::new(vec![]);
    let result = index(&db, UID, &empty, |_, _| {}).await?;
    assert!(result.changed);
    assert!(db.tracks_by_user(UID)?.is_empty());
    assert_eq!(empty.hashes.lock().unwrap().as_slice(), &[0, -1, 0]);
    assert!(empty.calls.lock().unwrap().is_empty());
    let unchanged = Source::new(vec![]);
    assert!(!index(&db, UID, &unchanged, |_, _| {}).await?.changed);
    Ok(())
}

#[tokio::test]
async fn account_change_before_next_commit_stops_the_scan() -> AppResult<()> {
    let db = active_db()?;
    let source = Source::new((1..=103).collect());
    assert!(index(&db, UID, &source, |_, done| {
        if done == 100 {
            db.clear_active_profile(UID).unwrap();
        }
    })
    .await
    .is_err());
    assert_eq!(db.tracks_by_user(UID)?.len(), 100);
    assert!(!db.saved_music_checkpoint(UID)?.unwrap().complete);
    assert!(db.load_profile()?.is_none());
    Ok(())
}

#[test]
fn page_transaction_rolls_back_metadata_and_cursor_on_invalid_entry() -> AppResult<()> {
    let db = active_db()?;
    db.begin_saved_music_scan(UID, &[1, 2], compute_saved_music_hash(&[1, 2]))?;
    assert!(db
        .append_saved_music_page(UID, 0, &[track(1, 0), track(2, 99)])
        .is_err());
    assert!(db.tracks_by_user(UID)?.is_empty());
    assert_eq!(db.saved_music_checkpoint(UID)?.unwrap().next_offset, 0);
    assert!(db.finish_saved_music_scan(UID).is_err());
    Ok(())
}

#[test]
fn local_pages_reject_mixed_revisions_and_invalid_bounds() -> AppResult<()> {
    let db = active_db()?;
    db.begin_saved_music_scan(UID, &[1, 2], compute_saved_music_hash(&[1, 2]))?;
    db.append_saved_music_page(UID, 0, &[track(1, 0)])?;
    let page = db.tracks_page(UID, 0, 1, None)?;
    db.append_saved_music_page(UID, 1, &[track(2, 1)])?;
    assert!(db
        .tracks_page(UID, 1, 1, Some(page.status.revision))
        .is_err());
    assert!(db.tracks_page(UID, -1, 100, None).is_err());
    assert!(db.tracks_page(UID, 0, 501, None).is_err());
    assert!(db.tracks_page(UID, 0, 0, None).is_err());
    assert!(db.tracks_page(99, 0, 100, None)?.tracks.is_empty());
    Ok(())
}

#[tokio::test]
async fn soft_sync_of_large_library_fetches_only_missing_metadata() -> AppResult<()> {
    let db = active_db()?;
    let old_ids: Vec<_> = (1..=10_037).collect();
    index(&db, UID, &Source::new(old_ids.clone()), |_, _| {}).await?;
    let first = db.tracks_by_user(UID)?[0].id;
    let mut cached = track(1, 0);
    cached.title = Some("Keep cached metadata".into());
    db.upsert_track(&cached)?;
    db.create_playlist_with_tracks(UID, "Keep duplicates", &[first, first])?;
    let mut ids = vec![10_038];
    ids.extend(old_ids);
    let source = Source::new(ids);
    let mut published = vec![];
    let result = soft_index(&db, UID, &source, |batch, _| {
        assert!(!batch.status.blocking);
        assert!(!db.tracks_page(UID, 0, 500, None).unwrap().tracks.is_empty());
        published.extend(batch.tracks.iter().map(|t| t.track.file_unique_id.clone()));
    })
    .await?;
    assert!(result.changed);
    assert_eq!(source.calls.lock().unwrap().as_slice(), &[0]);
    assert_eq!(published, vec!["10038"]);
    let tracks = db.tracks_by_user(UID)?;
    assert_eq!(tracks.len(), 10_038);
    assert_eq!(tracks[0].file_unique_id, "10038");
    assert_eq!(
        db.track_by_id(first, UID)?.unwrap().title.as_deref(),
        Some("Keep cached metadata")
    );
    assert_eq!(
        db.playlists_bundle(UID)?.custom[0].track_ids,
        vec![first, first]
    );
    Ok(())
}

#[tokio::test]
async fn soft_sync_unchanged_library_does_not_refresh_known_tracks() -> AppResult<()> {
    let db = active_db()?;
    index(&db, UID, &Source::new(vec![1, 2, 3]), |_, _| {}).await?;
    let mut cached = track(1, 0);
    cached.title = Some("Keep known title".into());
    db.upsert_track(&cached)?;
    let source = Source::new(vec![1, 2, 3]);
    assert!(!soft_index(&db, UID, &source, |_, _| {}).await?.changed);
    assert!(source.calls.lock().unwrap().is_empty());
    assert_eq!(
        db.tracks_by_user(UID)?[0].title.as_deref(),
        Some("Keep known title")
    );
    Ok(())
}

#[tokio::test]
async fn soft_sync_uses_distant_page_hints_and_falls_back_when_order_differs() -> AppResult<()> {
    let db = active_db()?;
    index(&db, UID, &Source::new((1..=150).collect()), |_, _| {}).await?;
    let mut source = Source::new((1..=151).collect());
    source.page_ids = vec![151];
    source.page_ids.extend(1..=150);
    soft_index(&db, UID, &source, |_, _| {}).await?;
    assert_eq!(source.calls.lock().unwrap().as_slice(), &[100, 0]);
    assert_eq!(db.tracks_by_user(UID)?[0].file_unique_id, "151");
    Ok(())
}

#[tokio::test]
async fn interrupted_soft_sync_resumes_only_still_missing_documents() -> AppResult<()> {
    let db = active_db()?;
    index(&db, UID, &Source::new((1..=203).collect()), |_, _| {}).await?;
    let mut failing = Source::new((1..=303).collect());
    failing.fail_at = Some(300);
    assert!(soft_index(&db, UID, &failing, |_, _| {}).await.is_err());
    assert_eq!(failing.calls.lock().unwrap().as_slice(), &[200, 300]);
    assert!(!db.library_status(UID)?.blocking);
    assert_eq!(db.tracks_page(UID, 0, 500, None)?.tracks.len(), 300);
    let resumed = Source::new(failing.ids.clone());
    soft_index(&db, UID, &resumed, |_, _| {}).await?;
    assert_eq!(resumed.calls.lock().unwrap().as_slice(), &[300]);
    assert_eq!(db.tracks_by_user(UID)?.len(), 303);
    Ok(())
}

#[tokio::test]
async fn soft_sync_continues_short_pages_without_overwriting_cached_metadata() -> AppResult<()> {
    let db = active_db()?;
    index(&db, UID, &Source::new(vec![]), |_, _| {}).await?;
    let mut source = Source::new((1..=19).collect());
    source.page_size = 7;
    soft_index(&db, UID, &source, |_, _| {}).await?;
    assert_eq!(source.calls.lock().unwrap().as_slice(), &[0, 7, 14]);
    assert_eq!(db.tracks_by_user(UID)?.len(), 19);
    assert!(!db.library_status(UID)?.blocking);
    Ok(())
}

#[tokio::test]
async fn soft_sync_prunes_only_after_final_manifest_verification() -> AppResult<()> {
    let db = active_db()?;
    index(&db, UID, &Source::new((1..=201).collect()), |_, _| {}).await?;
    let first = db.tracks_by_user(UID)?[0].id;
    db.create_playlist_with_tracks(UID, "Keep until verified", &[first])?;
    let mut source = Source::new((1..=202).collect());
    source.final_ids = Some((2..=203).collect());
    assert!(soft_index(&db, UID, &source, |_, _| {}).await.is_err());
    assert!(db.track_by_id(first, UID)?.is_some());
    assert_eq!(db.playlists_bundle(UID)?.custom[0].track_ids, vec![first]);
    let resumed = Source::new((2..=203).collect());
    soft_index(&db, UID, &resumed, |_, _| {}).await?;
    assert_eq!(resumed.calls.lock().unwrap().as_slice(), &[200]);
    assert!(db.track_by_id(first, UID)?.is_none());
    Ok(())
}

#[tokio::test]
async fn removal_only_soft_sync_needs_no_metadata_requests() -> AppResult<()> {
    let db = active_db()?;
    index(&db, UID, &Source::new(vec![1, 2, 3]), |_, _| {}).await?;
    let source = Source::new(vec![1, 3]);
    soft_index(&db, UID, &source, |_, _| {}).await?;
    assert!(source.calls.lock().unwrap().is_empty());
    assert_eq!(
        db.tracks_by_user(UID)?
            .iter()
            .map(|t| t.file_unique_id.as_str())
            .collect::<Vec<_>>(),
        vec!["1", "3"]
    );
    assert!(!db.library_status(UID)?.blocking);
    Ok(())
}

#[tokio::test]
async fn confirmed_full_reindex_remains_hidden_after_database_reopen() -> AppResult<()> {
    let path = std::env::temp_dir().join(format!(
        "soundgrammy-full-reindex-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let db = Db::open(&path)?;
        db.save_profile(UID, "Test", None, None, None)?;
        index(&db, UID, &Source::new(vec![1, 2, 3]), |_, _| {}).await?;
        let batch = db.request_full_reindex(UID)?;
        assert!(batch.status.blocking && !batch.status.complete);
        assert_eq!(batch.status.processed, 0);
        assert_eq!(db.tracks_by_user(UID)?.len(), 3);
    }
    {
        let db = Db::open(&path)?;
        assert!(db.library_status(UID)?.blocking);
        assert!(db.tracks_page(UID, 0, 500, None)?.tracks.is_empty());
        let source = Source::new(vec![1, 2, 3]);
        index(&db, UID, &source, |_, _| {}).await?;
        assert_eq!(source.calls.lock().unwrap().as_slice(), &[0]);
        assert_eq!(db.tracks_page(UID, 0, 500, None)?.tracks.len(), 3);
    }
    std::fs::remove_file(path)?;
    Ok(())
}
