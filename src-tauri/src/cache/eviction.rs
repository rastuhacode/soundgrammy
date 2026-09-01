use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tauri::AppHandle;
use tokio::fs;

use super::{audio_dir, current_uid, emit_cache_changed, get_cache_settings, usage_bytes};
use crate::db::AudioCacheClass;
use crate::error::AppResult;
use crate::state::AppState;

struct AudioCacheEntry {
    path: PathBuf,
    track_id: Option<i64>,
    size: u64,
    modified: SystemTime,
    pinned: bool,
    last_accessed_at_ms: Option<i64>,
}

struct PartialCacheEntry {
    path: PathBuf,
    metadata_path: PathBuf,
    metadata_temp_path: PathBuf,
    received: u64,
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
    let cache_records = state.db.audio_cache_entries()?;
    let mut entries = fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let meta = entry.metadata().await?;
        if !meta.is_file() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".part")
            || name.ends_with(".part.meta")
            || name.ends_with(".part.meta.tmp")
            || name.ends_with(".complete")
        {
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
        let record = track_id.and_then(|id| cache_records.get(&id));
        out.push(AudioCacheEntry {
            path,
            track_id,
            size: meta.len(),
            modified,
            pinned: record.is_some_and(|record| record.class == AudioCacheClass::Pinned),
            last_accessed_at_ms: record.map(|record| record.last_accessed_at_ms),
        });
    }
    Ok(out)
}

async fn list_partial_entries(state: &AppState) -> AppResult<Vec<PartialCacheEntry>> {
    let dir = audio_dir(state);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut entries = fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        if !name.to_string_lossy().ends_with(".part") {
            continue;
        }
        let path = entry.path();
        let Some(info) = crate::streaming::partial_cache_info(&path).await else {
            continue;
        };
        let modified = entry
            .metadata()
            .await?
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut metadata = path.as_os_str().to_owned();
        metadata.push(".meta");
        let metadata_path = PathBuf::from(metadata);
        let mut temp = path.as_os_str().to_owned();
        temp.push(".meta.tmp");
        out.push(PartialCacheEntry {
            path,
            metadata_path,
            metadata_temp_path: PathBuf::from(temp),
            received: info.received,
            modified,
        });
    }
    Ok(out)
}

pub(super) fn path_is_protected(path: &Path, protected: &HashSet<PathBuf>) -> bool {
    if protected.contains(path) {
        return true;
    }
    // Compare by canonical-ish string in case of relative vs absolute.
    let path_s = path.to_string_lossy();
    protected.iter().any(|p| p.to_string_lossy() == path_s)
}

pub(super) async fn protected_path_set(state: &AppState) -> HashSet<PathBuf> {
    state
        .streaming
        .protected_audio_paths()
        .await
        .into_iter()
        .collect()
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
        if entry.pinned || path_is_protected(&entry.path, &protected) {
            continue;
        }
        let accessed = entry
            .last_accessed_at_ms
            .and_then(|value| {
                SystemTime::UNIX_EPOCH
                    .checked_add(std::time::Duration::from_millis(value.max(0) as u64))
            })
            .unwrap_or(entry.modified);
        let age = match now.duration_since(accessed) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if age > ttl && fs::remove_file(&entry.path).await.is_ok() {
            if let Some(id) = entry.track_id {
                removed_ids.push(id);
                let _ = state.db.remove_audio_cache_entry(id);
            }
        }
    }

    for entry in list_partial_entries(state).await? {
        if path_is_protected(&entry.path, &protected) {
            continue;
        }
        let age = match now.duration_since(entry.modified) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if age > ttl {
            let _ = fs::remove_file(&entry.path).await;
            let _ = fs::remove_file(&entry.metadata_path).await;
            let _ = fs::remove_file(&entry.metadata_temp_path).await;
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

    let mut partials = list_partial_entries(state).await?;
    partials.retain(|entry| !path_is_protected(&entry.path, &protected));
    partials.sort_by_key(|entry| entry.modified);

    let mut freed = 0u64;
    for entry in partials {
        if freed >= to_free {
            break;
        }
        if fs::remove_file(&entry.path).await.is_ok() {
            let _ = fs::remove_file(&entry.metadata_path).await;
            let _ = fs::remove_file(&entry.metadata_temp_path).await;
            freed = freed.saturating_add(entry.received);
        }
    }

    let mut entries = list_audio_entries(state).await?;
    entries.retain(|e| !e.pinned && !path_is_protected(&e.path, &protected));
    // Higher score = older * heavier → evict first.
    entries.sort_by(|a, b| {
        let score = |e: &AudioCacheEntry| {
            let accessed = e
                .last_accessed_at_ms
                .and_then(|value| {
                    SystemTime::UNIX_EPOCH
                        .checked_add(std::time::Duration::from_millis(value.max(0) as u64))
                })
                .unwrap_or(e.modified);
            let age = now
                .duration_since(accessed)
                .unwrap_or_default()
                .as_secs_f64();
            age * e.size as f64
        };
        score(b)
            .partial_cmp(&score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
                    let _ = state.db.remove_audio_cache_entry(id);
                }
            }
            Err(_) => continue,
        }
    }

    if !removed_ids.is_empty() {
        emit_cache_changed(app, &removed_ids, false);
    }
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
        if entry.pinned || path_is_protected(&entry.path, &protected) {
            protected_used = protected_used.saturating_add(entry.size);
        }
    }
    for entry in list_partial_entries(state).await? {
        if path_is_protected(&entry.path, &protected) {
            protected_used = protected_used.saturating_add(entry.received);
        }
    }
    // After eviction we must keep protected files; room for new data is limit - protected.
    Ok(limit.saturating_sub(protected_used))
}
