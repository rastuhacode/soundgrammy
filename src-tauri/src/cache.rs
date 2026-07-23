//! Content-addressed on-disk cache for audio, thumbnails, and the avatar.
//!
//! Files are downloaded once (to a `.part` temp then atomically renamed) and
//! then served straight from disk through Tauri's asset protocol. Concurrent
//! plays of the same track are de-duplicated via per-key locks.
//!
//! Audio cache is subject to a size limit and TTL (see Settings). User Downloads
//! exports live outside this directory and are never touched by eviction/clear.

#![allow(deprecated)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use grammers_tl_types as tl;
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
use crate::telegram::{auth, download};

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
    Ok(audio_dir(state).join(format!("{}.{}", track.file_unique_id, ext)))
}

/// Ensures the track's audio is cached locally and returns its absolute path.
pub async fn ensure_audio(state: &AppState, app: &AppHandle, track_id: i64) -> AppResult<PathBuf> {
    let track = require_track(state, track_id)?;
    let dest = audio_path(state, &track)?;
    if dest.exists() {
        return Ok(dest);
    }

    // Make room for this one track when possible (explicit single-cache path).
    if let Some(size) = track.file_size {
        let _ = evict_for_room(state, app, size as u64).await;
    }

    tokio::fs::create_dir_all(audio_dir(state)).await?;
    let stream = state
        .streaming
        .start(app.clone(), track, dest.clone())
        .await?;
    let path = stream.wait_complete().await?;
    emit_cache_changed(app, &[track_id], true);
    Ok(path)
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
    let tracks = state.db.tracks_by_user(uid)?;
    let mut out = Vec::new();
    for track in tracks {
        let path = audio_path(state, &track)?;
        if path.exists() {
            out.push(track.id);
        }
    }
    Ok(out)
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
            if name.ends_with(".part") || name.ends_with(".complete") {
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

struct AudioCacheEntry {
    path: PathBuf,
    track_id: Option<i64>,
    size: u64,
    modified: SystemTime,
}

async fn list_audio_entries(state: &AppState) -> AppResult<Vec<AudioCacheEntry>> {
    let dir = audio_dir(state);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let uid = current_uid(state).ok();
    let mut by_unique: HashMap<String, i64> = HashMap::new();
    if let Some(uid) = uid {
        for track in state.db.tracks_by_user(uid)? {
            by_unique.insert(track.file_unique_id.clone(), track.id);
        }
    }

    let mut out = Vec::new();
    let mut entries = fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let meta = entry.metadata().await?;
        if !meta.is_file() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".part") || name.ends_with(".complete") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if stem.is_empty() {
            continue;
        }
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let track_id = by_unique.get(&stem).copied();
        out.push(AudioCacheEntry {
            path,
            track_id,
            size: meta.len(),
            modified,
        });
    }
    Ok(out)
}

fn path_is_protected(path: &Path, protected: &HashSet<PathBuf>) -> bool {
    if protected.contains(path) {
        return true;
    }
    // Compare by canonical-ish string in case of relative vs absolute.
    let path_s = path.to_string_lossy();
    protected.iter().any(|p| p.to_string_lossy() == path_s)
}

async fn protected_path_set(state: &AppState) -> HashSet<PathBuf> {
    state
        .streaming
        .protected_audio_paths()
        .await
        .into_iter()
        .collect()
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

fn emit_cache_cleared(app: &AppHandle) {
    let _ = app.emit(
        "cache:changed",
        CacheChanged {
            track_ids: Vec::new(),
            cached: false,
            cleared: true,
        },
    );
}

/// Remove audio older than the configured TTL (skipping active playback files).
pub async fn enforce_ttl(state: &AppState, app: &AppHandle) -> AppResult<Vec<i64>> {
    let settings = get_cache_settings(state)?;
    if settings.ttl_secs <= 0 {
        return Ok(Vec::new());
    }
    let ttl = std::time::Duration::from_secs(settings.ttl_secs as u64);
    let now = SystemTime::now();
    let protected = protected_path_set(state).await;
    let mut removed_ids = Vec::new();

    for entry in list_audio_entries(state).await? {
        if path_is_protected(&entry.path, &protected) {
            continue;
        }
        let age = match now.duration_since(entry.modified) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if age > ttl {
            if fs::remove_file(&entry.path).await.is_ok() {
                if let Some(id) = entry.track_id {
                    removed_ids.push(id);
                }
            }
        }
    }

    if !removed_ids.is_empty() {
        emit_cache_changed(app, &removed_ids, false);
    }
    Ok(removed_ids)
}

/// Free at least `needed` bytes under the limit by evicting older+heavier files first.
/// Returns bytes actually freed.
pub async fn evict_for_room(state: &AppState, app: &AppHandle, needed: u64) -> AppResult<u64> {
    let settings = get_cache_settings(state)?;
    let limit = settings.limit_bytes.max(0) as u64;
    let (used, _) = usage_bytes(state).await?;
    let available = limit.saturating_sub(used);
    if needed <= available {
        return Ok(0);
    }
    let to_free = needed.saturating_sub(available);
    let protected = protected_path_set(state).await;
    let now = SystemTime::now();

    let mut entries = list_audio_entries(state).await?;
    entries.retain(|e| !path_is_protected(&e.path, &protected));
    // Higher score = older * heavier → evict first.
    entries.sort_by(|a, b| {
        let score = |e: &AudioCacheEntry| {
            let age = now
                .duration_since(e.modified)
                .unwrap_or_default()
                .as_secs_f64();
            age * e.size as f64
        };
        score(b)
            .partial_cmp(&score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut freed = 0u64;
    let mut removed_ids = Vec::new();
    for entry in entries {
        if freed >= to_free {
            break;
        }
        match fs::remove_file(&entry.path).await {
            Ok(()) => {
                freed = freed.saturating_add(entry.size);
                if let Some(id) = entry.track_id {
                    removed_ids.push(id);
                }
            }
            Err(_) => continue,
        }
    }

    if !removed_ids.is_empty() {
        emit_cache_changed(app, &removed_ids, false);
    }
    let _ = to_free;
    Ok(freed)
}

/// How much room can be made for a new job (limit − protected bytes).
pub async fn available_after_eviction(state: &AppState) -> AppResult<u64> {
    let settings = get_cache_settings(state)?;
    let limit = settings.limit_bytes.max(0) as u64;
    let protected = protected_path_set(state).await;
    let entries = list_audio_entries(state).await?;
    let mut protected_used = 0u64;
    for entry in &entries {
        if path_is_protected(&entry.path, &protected) {
            protected_used = protected_used.saturating_add(entry.size);
        }
    }
    // After eviction we must keep protected files; room for new data is limit - protected.
    Ok(limit.saturating_sub(protected_used))
}

pub async fn remove_audio(state: &AppState, app: &AppHandle, track_id: i64) -> AppResult<()> {
    let track = require_track(state, track_id)?;
    let path = audio_path(state, &track)?;
    let protected = protected_path_set(state).await;
    if path_is_protected(&path, &protected) {
        // Safe no-op: leave playing track on disk.
        return Ok(());
    }
    if path.exists() {
        fs::remove_file(&path).await?;
    }
    // Also clear any alternate extension variants for this unique id.
    let dir = audio_dir(state);
    if dir.exists() {
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&track.file_unique_id)
                && !name.ends_with(".part")
                && !name.ends_with(".complete")
            {
                let p = entry.path();
                if !path_is_protected(&p, &protected) {
                    let _ = fs::remove_file(&p).await;
                }
            }
        }
    }
    emit_cache_changed(app, &[track_id], false);
    Ok(())
}

pub async fn clear_audio_cache(state: &AppState, app: &AppHandle) -> AppResult<()> {
    let protected = protected_path_set(state).await;
    let dir = audio_dir(state);
    if !dir.exists() {
        emit_cache_cleared(app);
        return Ok(());
    }
    let mut entries = fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path_is_protected(&path, &protected) {
            continue;
        }
        let _ = fs::remove_file(&path).await;
    }
    emit_cache_cleared(app);
    Ok(())
}

/// Pre-check + cache all uncached tracks in `track_ids`.
/// When `job_id` is set, emits `cache_tracks:progress` after each attempt.
pub async fn cache_tracks(
    state: &AppState,
    app: &AppHandle,
    track_ids: &[i64],
    job_id: Option<&str>,
) -> AppResult<Vec<i64>> {
    let uid = current_uid(state)?;
    let mut needed: u64 = 0;
    let mut to_cache: Vec<Track> = Vec::new();
    let mut unknown = Vec::new();

    for &id in track_ids {
        let track = state
            .db
            .track_by_id(id, uid)?
            .ok_or_else(|| AppError::msg(format!("track not found: {id}")))?;
        let path = audio_path(state, &track)?;
        if path.exists() {
            continue;
        }
        match track.file_size {
            Some(size) if size > 0 => {
                needed = needed.saturating_add(size as u64);
                to_cache.push(track);
            }
            _ => unknown.push(id),
        }
    }

    if !unknown.is_empty() {
        return Err(AppError::msg(format!(
            "Cannot cache: {} track(s) have unknown size. Open Settings to clear space or try again after sync.",
            unknown.len()
        )));
    }

    if to_cache.is_empty() {
        return Ok(Vec::new());
    }

    let settings = get_cache_settings(state)?;
    let limit = settings.limit_bytes.max(0) as u64;
    let available = available_after_eviction(state).await?;
    if needed > available {
        let (used, _) = usage_bytes(state).await?;
        return Err(AppError::msg(format!(
            "Cannot cache: need {}, available under limit {} (limit {}, currently using {}). Raise the cache limit or clear cache in Settings.",
            format_bytes(needed),
            format_bytes(available),
            format_bytes(limit),
            format_bytes(used),
        )));
    }

    // Evict enough room for the whole job upfront.
    let (used, _) = usage_bytes(state).await?;
    let free_now = limit.saturating_sub(used);
    if needed > free_now {
        evict_for_room(state, app, needed).await?;
    }

    // Progress totals match the requested playlist size (already-cached tracks
    // count as done) so the toolbar does not jump from `0/N` to `1/M`.
    let total = track_ids.len() as u32;
    let already_present = total.saturating_sub(to_cache.len() as u32);
    let mut cached = Vec::new();
    for (index, track) in to_cache.into_iter().enumerate() {
        if let Some(job_id) = job_id {
            let _ = app.emit(
                "cache_tracks:progress",
                CacheTracksProgress {
                    job_id: job_id.to_string(),
                    current: already_present.saturating_add(index as u32).saturating_add(1),
                    total,
                    track_id: track.id,
                },
            );
        }
        match ensure_audio(state, app, track.id).await {
            Ok(_) => cached.push(track.id),
            Err(err) => {
                if cached.is_empty() {
                    return Err(err);
                }
                // Partial success: stop and report.
                return Err(AppError::msg(format!(
                    "Caching stopped after {} track(s): {err}",
                    cached.len()
                )));
            }
        }
    }
    Ok(cached)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheTracksProgress {
    job_id: String,
    current: u32,
    total: u32,
    track_id: i64,
}

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

/// Ensures the track's thumbnail is cached and returns its path, if any exists.
///
/// Standard quality prefers the remote Telegram thumb, then embedded cover from
/// a cached audio file. High quality (fullscreen) prefers embedded cover when
/// the audio is already cached (`{id}.embed.jpg`), then falls back to the remote
/// thumb (`{id}.full.jpg`). Re-checking embedded art on later calls upgrades a
/// previously cached remote full thumb once the audio file appears.
pub async fn ensure_thumbnail(
    state: &AppState,
    track_id: i64,
    high_quality: bool,
) -> AppResult<Option<PathBuf>> {
    let track = require_track(state, track_id)?;
    let thumbs = thumb_dir(state);

    if high_quality {
        return ensure_high_quality_thumbnail(state, &track, &thumbs).await;
    }

    let dest = thumbs.join(format!("{}.jpg", track.file_unique_id));
    if dest.exists() {
        return Ok(Some(dest));
    }

    tokio::fs::create_dir_all(&thumbs).await?;

    let key = format!("thumb:{}:false", track.file_unique_id);
    let lock = state.lock_for(&key).await;
    let _guard = lock.lock().await;

    if dest.exists() {
        return Ok(Some(dest));
    }

    if download::download_thumbnail(state, &track, &dest, false).await? {
        return Ok(Some(dest));
    }

    let audio = audio_path(state, &track)?;
    if audio.exists() {
        if let Some(bytes) = extract_embedded_cover(&audio) {
            tokio::fs::write(&dest, &bytes).await?;
            return Ok(Some(dest));
        }
    }

    Ok(None)
}

async fn ensure_high_quality_thumbnail(
    state: &AppState,
    track: &Track,
    thumbs: &Path,
) -> AppResult<Option<PathBuf>> {
    let embed_dest = thumbs.join(format!("{}.embed.jpg", track.file_unique_id));
    let full_dest = thumbs.join(format!("{}.full.jpg", track.file_unique_id));

    if embed_dest.exists() {
        return Ok(Some(embed_dest));
    }

    tokio::fs::create_dir_all(thumbs).await?;

    let key = format!("thumb:{}:true", track.file_unique_id);
    let lock = state.lock_for(&key).await;
    let _guard = lock.lock().await;

    if embed_dest.exists() {
        return Ok(Some(embed_dest));
    }

    let audio = audio_path(state, track)?;
    if audio.exists() {
        if let Some(bytes) = extract_embedded_cover(&audio) {
            tokio::fs::write(&embed_dest, &bytes).await?;
            return Ok(Some(embed_dest));
        }
    }

    if full_dest.exists() {
        return Ok(Some(full_dest));
    }

    if download::download_thumbnail(state, track, &full_dest, true).await? {
        return Ok(Some(full_dest));
    }

    Ok(None)
}

/// Ensures the current user's avatar is cached and returns its path, if any.
pub async fn ensure_avatar(state: &AppState) -> AppResult<Option<PathBuf>> {
    let user = auth::fetch_self_raw(&state.client).await?;
    let photo = match user.photo {
        Some(tl::enums::UserProfilePhoto::Photo(p)) => p,
        _ => return Ok(None),
    };

    let dest = thumb_dir(state).join(format!("avatar_{}_{}.jpg", user.id, photo.photo_id));
    if dest.exists() {
        return Ok(Some(dest));
    }

    tokio::fs::create_dir_all(thumb_dir(state)).await?;

    let key = format!("avatar:{}", photo.photo_id);
    let lock = state.lock_for(&key).await;
    let _guard = lock.lock().await;

    if dest.exists() {
        return Ok(Some(dest));
    }

    let location = tl::enums::InputFileLocation::InputPeerPhotoFileLocation(
        tl::types::InputPeerPhotoFileLocation {
            big: false,
            peer: tl::enums::InputPeer::PeerSelf,
            photo_id: photo.photo_id,
        },
    );

    download::download_location(&state.client, location, &dest).await?;
    Ok(Some(dest))
}

/// Reads the first embedded picture from an audio file, if present.
fn extract_embedded_cover(path: &Path) -> Option<Vec<u8>> {
    use lofty::file::TaggedFileExt;
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag.pictures().first()?;
    Some(picture.data().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

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
}
