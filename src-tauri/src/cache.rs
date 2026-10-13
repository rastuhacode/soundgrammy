//! Content-addressed on-disk cache for audio, thumbnails, and the avatar.
//!
//! Files are downloaded once (to a `.part` temp then atomically renamed) and
//! then served straight from disk through Tauri's asset protocol. Concurrent
//! plays of the same track are de-duplicated via per-key locks.
//!
//! Audio cache is subject to a size limit and TTL (see Settings). User Downloads
//! exports live outside this directory and are never touched by eviction/clear.

#![allow(deprecated)]

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::fs;

use crate::db::{
    Track, DEFAULT_CACHE_LIMIT_BYTES, DEFAULT_CACHE_TTL_SECS, SETTING_CACHE_LIMIT_BYTES,
    SETTING_CACHE_TTL_SECS,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::extension_for_mime;
use crate::telegram::download;

mod eviction;
mod leases;
pub use leases::{AudioLease, BudgetRegistry, CacheLease, LeaseRegistry};
mod media_art;

pub use eviction::{available_after_eviction, enforce_ttl, evict_for_room};
pub use media_art::{ensure_avatar, ensure_thumbnail, schedule_embedded_artwork};

use eviction::{path_is_protected, protected_path_set};

fn audio_dir(state: &AppState) -> PathBuf {
    state.cache_dir.join("audio")
}

fn thumb_dir(state: &AppState) -> PathBuf {
    state.cache_dir.join("thumbs")
}

fn current_uid(state: &AppState) -> AppResult<i64> {
    state
        .db
        .load_profile()?
        .map(|p| p.tg_user_id)
        .ok_or(AppError::NotAuthorized)
}

pub(crate) fn require_track(state: &AppState, track_id: i64) -> AppResult<Track> {
    let uid = current_uid(state)?;
    state
        .db
        .track_by_id(track_id, uid)?
        .ok_or_else(|| AppError::msg("track not found"))
}

pub(crate) fn audio_path(state: &AppState, track: &Track) -> AppResult<PathBuf> {
    let doc = download::stored_document(track)?;
    let ext = extension_for_mime(&doc.mime_type);
    let directory = audio_dir(state);
    let expected = directory.join(format!("{}.{}", track.file_unique_id, ext));
    if expected.exists() {
        return Ok(expected);
    }
    for extension in ["mp3", "m4a", "ogg", "opus", "flac", "wav", "webm", "bin"] {
        let candidate = directory.join(format!("{}.{}", track.file_unique_id, extension));
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Ok(expected)
}

/// Ensures the track's audio is cached locally and returns its absolute path.
pub async fn ensure_audio(state: &AppState, app: &AppHandle, track_id: i64) -> AppResult<PathBuf> {
    Ok(ensure_audio_leased(state, app, track_id)
        .await?
        .path
        .clone())
}

pub async fn ensure_audio_leased(
    state: &AppState,
    app: &AppHandle,
    track_id: i64,
) -> AppResult<AudioLease> {
    let track = require_track(state, track_id)?;
    let dest = audio_path(state, &track)?;
    let lease = {
        let _guard = state.cache_lifecycle.lock().await;
        state.cache_leases.acquire(&track.file_unique_id)
    };
    if dest.exists() {
        let _ = state.db.touch_audio_cache(track_id);
        state.audio.cache_ready(app.clone(), track_id).await;
        return Ok(AudioLease {
            path: dest,
            _lease: lease,
        });
    }

    tokio::fs::create_dir_all(audio_dir(state)).await?;
    let stream = state
        .streaming
        .start(app.clone(), track, dest.clone())
        .await?;
    state
        .audio
        .observe_cache(app.clone(), track_id, stream.clone())
        .await;
    // Start first so a resumable partial for this track is protected from the
    // eviction pass, and reserve only the bytes it is still missing.
    let remaining = stream.total().saturating_sub(stream.received().await);
    let needed = if state
        .cache_budget
        .snapshot()
        .contains_key(&stream.cache_key())
    {
        0
    } else {
        remaining
    };
    evict_for_room(state, app, needed).await?;
    let path = stream.download_complete().await?;
    state.audio.cache_ready(app.clone(), track_id).await;
    emit_cache_changed(app, &[track_id], true);
    Ok(AudioLease {
        path,
        _lease: lease,
    })
}

async fn cached_audio_lease(state: &AppState, track: &Track) -> AppResult<Option<AudioLease>> {
    let _guard = state.cache_lifecycle.lock().await;
    let path = audio_path(state, track)?;
    Ok(path.is_file().then(|| AudioLease {
        path,
        _lease: state.cache_leases.acquire(&track.file_unique_id),
    }))
}

/// Returns whether a track's audio file is fully present (kept for tests / callers).
#[allow(dead_code)]
pub fn is_audio_cached(state: &AppState, track_id: i64) -> AppResult<bool> {
    let track = require_track(state, track_id)?;
    let path = audio_path(state, &track)?;
    Ok(path.exists())
}

/// Returns track ids whose audio file is fully present in the app cache.
pub fn cached_track_ids(state: &AppState) -> AppResult<Vec<i64>> {
    let uid = current_uid(state)?;
    let directory = audio_dir(state);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut keys = std::collections::HashSet::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let parts: Vec<_> = name.split('.').collect();
        if parts.len() == 2 && !matches!(parts[1], "part" | "meta" | "tmp" | "complete") {
            keys.insert(parts[0].to_string());
        }
    }
    let mut ids = Vec::new();
    for (id, key) in state.db.cache_identities_for_user(uid)? {
        if keys.contains(&key) {
            ids.push(id);
        }
    }
    Ok(ids)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSettings {
    pub limit_bytes: i64,
    pub ttl_secs: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheUsage {
    pub used_bytes: u64,
    pub limit_bytes: i64,
    pub file_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheChanged {
    pub track_ids: Vec<i64>,
    pub cached: bool,
    pub cleared: bool,
}

pub fn get_cache_settings(state: &AppState) -> AppResult<CacheSettings> {
    Ok(CacheSettings {
        limit_bytes: state
            .db
            .get_setting_i64(SETTING_CACHE_LIMIT_BYTES, DEFAULT_CACHE_LIMIT_BYTES)?,
        ttl_secs: state
            .db
            .get_setting_i64(SETTING_CACHE_TTL_SECS, DEFAULT_CACHE_TTL_SECS)?,
    })
}

pub fn set_cache_settings(
    state: &AppState,
    limit_bytes: Option<i64>,
    ttl_secs: Option<i64>,
) -> AppResult<CacheSettings> {
    if let Some(limit) = limit_bytes {
        if limit < 0 {
            return Err(AppError::msg("cache limit must be non-negative"));
        }
        state
            .db
            .set_setting(SETTING_CACHE_LIMIT_BYTES, &limit.to_string())?;
    }
    if let Some(ttl) = ttl_secs {
        if ttl < 0 {
            return Err(AppError::msg("cache TTL must be non-negative"));
        }
        state
            .db
            .set_setting(SETTING_CACHE_TTL_SECS, &ttl.to_string())?;
    }
    get_cache_settings(state)
}

pub(super) fn allocated_bytes(meta: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.blocks().saturating_mul(512)
    }
    #[cfg(not(unix))]
    {
        meta.len()
    }
}

pub async fn usage_bytes(state: &AppState) -> AppResult<(u64, u64)> {
    let dir = audio_dir(state);
    if !dir.exists() {
        return Ok((0, 0));
    }
    let mut used = 0u64;
    let mut count = 0u64;
    let mut entries = fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let meta = entry.metadata().await?;
        if meta.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".part") {
                if let Some(info) = crate::streaming::partial_cache_info(&entry.path()).await {
                    used = used.saturating_add(info.received);
                } else {
                    used = used.saturating_add(allocated_bytes(&meta));
                }
                count += 1;
                continue;
            }
            if name.ends_with(".part.meta")
                || name.ends_with(".part.meta.tmp")
                || name.ends_with(".complete")
            {
                continue;
            }
            used = used.saturating_add(meta.len());
            count += 1;
        }
    }
    Ok((used, count))
}

pub async fn get_cache_usage(state: &AppState) -> AppResult<CacheUsage> {
    let settings = get_cache_settings(state)?;
    let (used_bytes, file_count) = usage_bytes(state).await?;
    Ok(CacheUsage {
        used_bytes,
        limit_bytes: settings.limit_bytes,
        file_count,
    })
}

fn emit_cache_changed(app: &AppHandle, track_ids: &[i64], cached: bool) {
    let _ = app.emit(
        "cache:changed",
        CacheChanged {
            track_ids: track_ids.to_vec(),
            cached,
            cleared: false,
        },
    );
}

fn matches_audio_identity(name: &str, unique_id: &str) -> bool {
    name.strip_prefix(unique_id)
        .is_some_and(|suffix| suffix.starts_with('.'))
}

pub async fn remove_audio(state: &AppState, app: &AppHandle, track_id: i64) -> AppResult<()> {
    let removed = remove_audio_document(state, track_id).await?;
    if !removed.is_empty() {
        emit_cache_changed(app, &removed, false);
    }
    Ok(())
}

async fn remove_audio_document(state: &AppState, track_id: i64) -> AppResult<Vec<i64>> {
    let _guard = state.cache_lifecycle.lock().await;
    let track = require_track(state, track_id)?;
    let protected = protected_path_set(state).await;
    let marker = audio_dir(state).join(&track.file_unique_id);
    if protected.contains(&marker) || path_is_protected(&audio_path(state, &track)?, &protected) {
        return Ok(Vec::new());
    }
    let dir = audio_dir(state);
    if dir.exists() {
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if matches_audio_identity(&entry.file_name().to_string_lossy(), &track.file_unique_id) {
                let path = entry.path();
                if path_is_protected(&path, &protected) {
                    return Ok(Vec::new());
                }
                match fs::remove_file(path).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    let removed = state
        .db
        .cache_identities()?
        .into_iter()
        .filter_map(|(id, key)| (key == track.file_unique_id).then_some(id))
        .collect::<Vec<_>>();
    for id in &removed {
        state.db.remove_audio_cache_entry(*id)?;
    }
    Ok(removed)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheClearResult {
    pub removed_track_ids: Vec<i64>,
    pub protected_track_ids: Vec<i64>,
    pub failed_files: usize,
}

pub async fn clear_audio_cache(state: &AppState, app: &AppHandle) -> AppResult<CacheClearResult> {
    let result = clear_audio_directory(state).await?;
    if !result.removed_track_ids.is_empty() {
        emit_cache_changed(app, &result.removed_track_ids, false);
    }
    Ok(result)
}

async fn clear_audio_directory(state: &AppState) -> AppResult<CacheClearResult> {
    let _guard = state.cache_lifecycle.lock().await;
    let protected = protected_path_set(state).await;
    let dir = audio_dir(state);
    let identities = state.db.cache_identities()?;
    let mut removed = std::collections::HashSet::new();
    let mut failures = 0;
    if dir.exists() {
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path_is_protected(&path, &protected) {
                continue;
            }
            match fs::remove_file(&path).await {
                Ok(()) => {
                    for (id, key) in &identities {
                        if matches_audio_identity(&entry.file_name().to_string_lossy(), key) {
                            removed.insert(*id);
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => failures += 1,
            }
        }
    }
    let mut remaining = Vec::new();
    if dir.exists() {
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            remaining.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    let mut protected_ids = Vec::new();
    for (id, key) in &identities {
        let protected_key = protected.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == key || matches_audio_identity(name, key))
        });
        let complete_exists = remaining
            .iter()
            .any(|name| matches_audio_identity(name, key) && name.split('.').count() == 2);
        if protected_key || complete_exists {
            if protected_key {
                protected_ids.push(*id);
            }
            removed.remove(id);
        } else {
            removed.insert(*id);
        }
    }
    let removed: Vec<_> = removed.into_iter().collect();
    for id in &removed {
        state.db.remove_audio_cache_entry(*id)?;
    }
    Ok(CacheClearResult {
        removed_track_ids: removed,
        protected_track_ids: protected_ids,
        failed_files: failures,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheJobResult {
    pub succeeded: Vec<i64>,
    pub failed: Vec<CacheJobFailure>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheJobFailure {
    pub track_id: i64,
    pub error: String,
}

struct CacheJobPlan {
    tracks: Vec<Track>,
    _leases: Vec<CacheLease>,
    _reservations: Vec<leases::Reservation>,
}
async fn prepare_cache_job(state: &AppState, track_ids: &[i64]) -> AppResult<CacheJobPlan> {
    let uid = current_uid(state)?;
    let mut unique = std::collections::HashSet::new();
    let tracks: Vec<_> = track_ids
        .iter()
        .filter(|id| unique.insert(**id))
        .map(|id| {
            state
                .db
                .track_by_id(*id, uid)?
                .ok_or_else(|| AppError::msg("Track not found"))
        })
        .collect::<AppResult<_>>()?;
    // Hold both reservations and leases through the complete job, including pin publication.
    let (leases, reservations) = {
        let _guard = state.cache_lifecycle.lock().await;
        let existing = state.cache_budget.snapshot();
        let mut leases = Vec::new();
        let mut needed = 0u64;
        let mut reservations = Vec::new();
        for track in &tracks {
            let path = audio_path(state, track)?;
            let size = if path.exists() {
                fs::metadata(&path).await?.len()
            } else {
                track
                    .file_size
                    .filter(|size| *size > 0)
                    .ok_or_else(|| AppError::msg("Cannot cache a track with unknown size"))?
                    as u64
            };
            leases.push(state.cache_leases.acquire(&track.file_unique_id));
            if !path.exists() && !existing.contains_key(&track.file_unique_id) {
                let partial = PathBuf::from(format!("{}.part", path.display()));
                let received = crate::streaming::partial_cache_info(&partial)
                    .await
                    .map_or(0, |i| i.received);
                needed = needed.saturating_add(size.saturating_sub(received));
            }
        }
        if needed > available_after_eviction(state).await? {
            return Err(AppError::msg(
                "Cannot cache: not enough room under the cache limit",
            ));
        }
        for track in &tracks {
            if !audio_path(state, track)?.exists() {
                reservations.push(
                    state
                        .cache_budget
                        .reserve(&track.file_unique_id, track.file_size.unwrap() as u64),
                );
            }
        }
        (leases, reservations)
    };
    Ok(CacheJobPlan {
        tracks,
        _leases: leases,
        _reservations: reservations,
    })
}

pub async fn cache_tracks(
    state: &AppState,
    app: &AppHandle,
    track_ids: &[i64],
    job_id: Option<&str>,
) -> AppResult<CacheJobResult> {
    let plan = prepare_cache_job(state, track_ids).await?;
    let tracks = &plan.tracks;
    let mut result = CacheJobResult {
        succeeded: Vec::new(),
        failed: Vec::new(),
    };
    for (index, track) in tracks.iter().enumerate() {
        if let Some(job_id) = job_id {
            let _ = app.emit(
                "cache_tracks:progress",
                CacheTracksProgress {
                    job_id: job_id.into(),
                    current: track_ids
                        .iter()
                        .filter(|id| tracks[..=index].iter().any(|t| t.id == **id))
                        .count() as u32,
                    total: track_ids.len() as u32,
                    track_id: track.id,
                },
            );
        }
        let outcome = async {
            let _lease = ensure_audio_leased(state, app, track.id).await?;
            state.db.mark_audio_cache_pinned(track.id)
        }
        .await;
        match outcome {
            Ok(()) => result.succeeded.push(track.id),
            Err(error) => result.failed.push(CacheJobFailure {
                track_id: track.id,
                error: error.to_string(),
            }),
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheTracksProgress {
    job_id: String,
    current: u32,
    total: u32,
    track_id: i64,
}

#[cfg(test)]
fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} GB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MB", b / MIB)
    } else if b >= KIB {
        format!("{:.0} KB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    #[test]
    fn format_bytes_uses_sensible_units() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(2048), "2 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GB");
    }

    #[test]
    fn path_is_protected_matches_exact_paths() {
        let playing = PathBuf::from("/tmp/cache/audio/abc.mp3");
        let protected: HashSet<PathBuf> = [playing.clone()].into_iter().collect();
        assert!(path_is_protected(&playing, &protected));
        assert!(!path_is_protected(
            Path::new("/tmp/cache/audio/other.mp3"),
            &protected
        ));
    }

    #[test]
    fn eviction_score_prefers_older_and_heavier() {
        use std::time::{Duration, SystemTime};

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        let score = |age_secs: u64, size: u64| -> f64 {
            let modified = now - Duration::from_secs(age_secs);
            let age = now
                .duration_since(modified)
                .unwrap_or_default()
                .as_secs_f64();
            age * size as f64
        };

        // Older+heavier should outrank newer+smaller.
        assert!(score(5_000, 10_000_000) > score(100, 1_000_000));
        // Same age: heavier wins.
        assert!(score(1_000, 8_000_000) > score(1_000, 2_000_000));
        // Same size: older wins.
        assert!(score(9_000, 1_000_000) > score(100, 1_000_000));
    }
    #[test]
    fn removing_audio_matches_exact_document_not_neighboring_prefixes() {
        for name in ["123.mp3", "123.webm", "123.mp3.part", "123.mp3.part.meta"] {
            assert!(matches_audio_identity(name, "123"));
        }
        for name in ["1234.mp3", "12345.mp3.part", "12.mp3", "123"] {
            assert!(!matches_audio_identity(name, "123"));
        }
    }

    #[tokio::test]
    async fn cache_clear_preserves_leased_pins_and_reports_failed_entries() {
        let state = test_state();
        let a = insert_track(&state, 1, "123");
        let b = insert_track(&state, 1, "124");
        state.db.save_profile(1, "User", None, None, None).unwrap();
        state.db.mark_audio_cache_pinned(a).unwrap();
        fs::create_dir_all(audio_dir(&state)).await.unwrap();
        fs::write(audio_dir(&state).join("123.mp3"), b"pinned")
            .await
            .unwrap();
        fs::write(audio_dir(&state).join("124.mp3"), b"remove")
            .await
            .unwrap();
        fs::create_dir(audio_dir(&state).join("cannot-remove-as-file"))
            .await
            .unwrap();
        let lease = state.cache_leases.acquire("123");
        let result = clear_audio_directory(&state).await.unwrap();
        assert_eq!(result.failed_files, 1);
        assert!(result.protected_track_ids.contains(&a));
        assert!(result.removed_track_ids.contains(&b));
        assert!(audio_dir(&state).join("123.mp3").exists());
        assert_eq!(
            state.db.audio_cache_entries().unwrap()[&a].class,
            crate::db::AudioCacheClass::Pinned
        );
        drop(lease);
        let result = clear_audio_directory(&state).await.unwrap();
        assert!(result.removed_track_ids.contains(&a));
        assert!(!state.db.audio_cache_entries().unwrap().contains_key(&a));
        fs::remove_dir_all(&state.cache_dir).await.unwrap();
    }

    #[tokio::test]
    async fn cache_admission_deduplicates_memberships_and_serializes_competing_jobs() {
        let state = test_state();
        state.db.save_profile(1, "User", None, None, None).unwrap();
        let a = insert_track(&state, 1, "123");
        let b = insert_track(&state, 1, "124");
        state
            .db
            .set_setting(SETTING_CACHE_LIMIT_BYTES, "12")
            .unwrap();
        let plan = prepare_cache_job(&state, &[a, a]).await.unwrap();
        assert_eq!(plan.tracks.len(), 1);
        assert_eq!(state.cache_budget.snapshot().values().sum::<u64>(), 12);
        assert!(prepare_cache_job(&state, &[b]).await.is_err());
        drop(plan);
        let first_ids = [a];
        let second_ids = [b];
        let (first, second) = tokio::join!(
            prepare_cache_job(&state, &first_ids),
            prepare_cache_job(&state, &second_ids)
        );
        assert_ne!(first.is_ok(), second.is_ok());
    }

    #[test]
    fn corrected_shared_file_is_available_to_another_accounts_old_mime_metadata() {
        let state = test_state();
        let a = insert_track(&state, 1, "123");
        let b = insert_track(&state, 2, "123");
        std::fs::create_dir_all(audio_dir(&state)).unwrap();
        std::fs::write(audio_dir(&state).join("123.webm"), b"finalized").unwrap();
        for (id, uid) in [(a, 1), (b, 2)] {
            let track = state.db.track_by_id(id, uid).unwrap().unwrap();
            assert_eq!(
                audio_path(&state, &track).unwrap().extension().unwrap(),
                "webm"
            );
        }
        state.db.save_profile(2, "B", None, None, None).unwrap();
        assert_eq!(cached_track_ids(&state).unwrap(), vec![b]);
        std::fs::remove_dir_all(&state.cache_dir).unwrap();
    }
    pub(crate) fn test_state() -> AppState {
        let directory = std::env::temp_dir().join(format!(
            "soundgrammy-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        AppState::new(
            crate::config::Config {
                api_id: 1,
                api_hash: "test".into(),
                lastfm: None,
            },
            crate::db::test_db().unwrap(),
            None,
            directory.clone(),
            directory,
            false,
            None,
        )
    }

    pub(crate) fn insert_track(state: &AppState, user: i64, key: &str) -> i64 {
        state
            .db
            .upsert_track(&crate::db::UpsertTrack {
                tg_user_id: user,
                file_id: key.into(),
                file_unique_id: key.into(),
                title: Some("Track".into()),
                title_source: "telegram_audio".into(),
                performer: Some("Artist".into()),
                duration: Some(60),
                mime_type: Some("audio/mpeg".into()),
                file_size: Some(12),
                track_position: 0,
                mtproto_document:
                    serde_json::json!({"id":key.parse::<i64>().unwrap(),"accessHash":0,
                "fileReference":"","dcId":1,"mimeType":"audio/mpeg","size":"12"})
                    .to_string(),
            })
            .unwrap();
        state
            .db
            .tracks_by_user(user)
            .unwrap()
            .into_iter()
            .find(|t| t.file_unique_id == key)
            .unwrap()
            .id
    }
    #[tokio::test]
    async fn removing_shared_document_updates_all_accounts_and_keeps_neighboring_document() {
        let state = test_state();
        state.db.save_profile(1, "User", None, None, None).unwrap();
        let a = insert_track(&state, 1, "123");
        let b = insert_track(&state, 2, "123");
        let neighbor = insert_track(&state, 1, "1234");
        fs::create_dir_all(audio_dir(&state)).await.unwrap();
        for key in ["123.mp3", "1234.mp3"] {
            fs::write(audio_dir(&state).join(key), b"audio")
                .await
                .unwrap();
        }
        for id in [a, b, neighbor] {
            state.db.mark_audio_cache_pinned(id).unwrap();
        }
        let lease = state.cache_leases.acquire("123");
        assert!(remove_audio_document(&state, a).await.unwrap().is_empty());
        drop(lease);
        let removed = remove_audio_document(&state, a).await.unwrap();
        assert!(removed.contains(&a) && removed.contains(&b));
        assert!(audio_dir(&state).join("1234.mp3").exists());
        let entries = state.db.audio_cache_entries().unwrap();
        assert!(!entries.contains_key(&a) && !entries.contains_key(&b));
        assert!(entries.contains_key(&neighbor));
        fs::remove_dir_all(&state.cache_dir).await.unwrap();
    }
}
