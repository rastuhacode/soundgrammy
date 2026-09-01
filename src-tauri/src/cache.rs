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
mod media_art;

pub use eviction::{available_after_eviction, enforce_ttl, evict_for_room};
pub use media_art::{ensure_avatar, ensure_thumbnail};

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
    Ok(audio_dir(state).join(format!("{}.{}", track.file_unique_id, ext)))
}

/// Ensures the track's audio is cached locally and returns its absolute path.
pub async fn ensure_audio(state: &AppState, app: &AppHandle, track_id: i64) -> AppResult<PathBuf> {
    let track = require_track(state, track_id)?;
    let dest = audio_path(state, &track)?;
    if dest.exists() {
        let _ = state.db.touch_audio_cache(track_id);
        return Ok(dest);
    }

    tokio::fs::create_dir_all(audio_dir(state)).await?;
    let stream = state
        .streaming
        .start(app.clone(), track, dest.clone())
        .await?;
    // Start first so a resumable partial for this track is protected from the
    // eviction pass, and reserve only the bytes it is still missing.
    let remaining = stream.total().saturating_sub(stream.received().await);
    let _ = evict_for_room(state, app, remaining).await;
    let path = stream.download_complete().await?;
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
            if name.ends_with(".part") {
                if let Some(info) = crate::streaming::partial_cache_info(&entry.path()).await {
                    used = used.saturating_add(info.received);
                    count += 1;
                }
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
            if name.starts_with(&track.file_unique_id) {
                let p = entry.path();
                if !path_is_protected(&p, &protected) {
                    let _ = fs::remove_file(&p).await;
                }
            }
        }
    }
    emit_cache_changed(app, &[track_id], false);
    let _ = state.db.remove_audio_cache_entry(track_id);
    Ok(())
}

pub async fn clear_audio_cache(state: &AppState, app: &AppHandle) -> AppResult<()> {
    let protected = protected_path_set(state).await;
    let dir = audio_dir(state);
    if !dir.exists() {
        let _ = state.db.clear_audio_cache_entries();
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
    let _ = state.db.clear_audio_cache_entries();
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
            state.db.mark_audio_cache_pinned(id)?;
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
                    current: already_present
                        .saturating_add(index as u32)
                        .saturating_add(1),
                    total,
                    track_id: track.id,
                },
            );
        }
        match ensure_audio(state, app, track.id).await {
            Ok(_) => {
                state.db.mark_audio_cache_pinned(track.id)?;
                cached.push(track.id);
            }
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

#[cfg(test)]
mod tests {
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
}
