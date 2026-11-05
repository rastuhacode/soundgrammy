use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tauri::AppHandle;
use tokio::fs;

use super::{audio_dir, emit_cache_changed, get_cache_settings, usage_bytes};
use crate::db::AudioCacheClass;
use crate::error::AppResult;
use crate::state::AppState;

struct AudioCacheEntry {
    path: PathBuf,
    track_ids: Vec<i64>,
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

    let mut by_unique: HashMap<String, Vec<i64>> = HashMap::new();
    for (id, key) in state.db.cache_identities()? {
        by_unique.entry(key).or_default().push(id);
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
        let ids = by_unique.get(&stem).cloned().unwrap_or_default();

        let records: Vec<_> = ids.iter().filter_map(|id| cache_records.get(id)).collect();
        out.push(AudioCacheEntry {
            path,
            track_ids: ids,
            size: meta.len(),
            modified,
            pinned: records
                .iter()
                .any(|record| record.class == AudioCacheClass::Pinned),
            last_accessed_at_ms: records
                .iter()
                .map(|record| record.last_accessed_at_ms)
                .max(),
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
        let metadata = entry.metadata().await?;
        let received = crate::streaming::partial_cache_info(&path)
            .await
            .map_or_else(|| super::allocated_bytes(&metadata), |info| info.received);
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let mut metadata = path.as_os_str().to_owned();
        metadata.push(".meta");
        let metadata_path = PathBuf::from(metadata);
        let mut temp = path.as_os_str().to_owned();
        temp.push(".meta.tmp");
        out.push(PartialCacheEntry {
            path,
            metadata_path,
            metadata_temp_path: PathBuf::from(temp),
            received,
            modified,
        });
    }
    Ok(out)
}

pub(super) fn path_is_protected(path: &Path, protected: &HashSet<PathBuf>) -> bool {
    if protected.contains(path) {
        return true;
    }
    let path_s = path.to_string_lossy();
    protected.iter().any(|p| {
        path_s
            .strip_prefix(p.to_string_lossy().as_ref())
            .is_some_and(|suffix| suffix.starts_with('.'))
    })
}

pub(super) async fn protected_path_set(state: &AppState) -> HashSet<PathBuf> {
    let mut paths: HashSet<PathBuf> = state
        .streaming
        .protected_audio_paths()
        .await
        .into_iter()
        .collect();
    paths.extend(state.audio.protected_paths());
    paths.extend(state.cache_leases.protected_paths(&audio_dir(state)));
    paths
}

async fn cleanup_orphans(state: &AppState, protected: &HashSet<PathBuf>) -> AppResult<()> {
    let dir = audio_dir(state);
    if !dir.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(&dir).await?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    let mut invalid_parts = HashSet::new();
    for name in &names {
        let path = dir.join(name);
        if name.ends_with(".part")
            && !path_is_protected(&path, protected)
            && crate::streaming::partial_cache_info(&path).await.is_none()
        {
            invalid_parts.insert(name.clone());
        }
    }
    for name in names {
        let path = dir.join(&name);
        if path_is_protected(&path, protected) {
            continue;
        }
        let orphan_sidecar = name
            .strip_suffix(".meta")
            .or_else(|| name.strip_suffix(".meta.tmp"))
            .is_some_and(|part| invalid_parts.contains(part) || !dir.join(part).exists());
        if invalid_parts.contains(&name) || orphan_sidecar || name.ends_with(".complete") {
            match fs::remove_file(&path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

/// Remove audio older than the configured TTL (skipping active playback files).
pub async fn enforce_ttl(state: &AppState, app: &AppHandle) -> AppResult<Vec<i64>> {
    let _guard = state.cache_lifecycle.lock().await;
    cleanup_orphans(state, &protected_path_set(state).await).await?;
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
            for id in entry.track_ids {
                removed_ids.push(id);
                state.db.remove_audio_cache_entry(id)?;
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
    let _guard = state.cache_lifecycle.lock().await;
    let settings = get_cache_settings(state)?;
    let limit = settings.limit_bytes.max(0) as u64;
    let (used, _) = usage_bytes(state).await?;
    let used = used.saturating_add(reserved_missing_bytes(state).await?);
    let to_free = used.saturating_add(needed).saturating_sub(limit);
    if to_free == 0 {
        return Ok(0);
    }
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
                for id in entry.track_ids {
                    removed_ids.push(id);
                    state.db.remove_audio_cache_entry(id)?;
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

async fn reserved_missing_bytes(state: &AppState) -> AppResult<u64> {
    let reservations = state.cache_budget.snapshot();
    if reservations.is_empty() {
        return Ok(0);
    }
    let mut present: HashMap<String, u64> = HashMap::new();
    for entry in list_audio_entries(state).await? {
        if let Some(key) = entry.path.file_stem().and_then(|s| s.to_str()) {
            present.insert(key.into(), entry.size);
        }
    }
    for entry in list_partial_entries(state).await? {
        if let Some(key) = entry
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('.').next())
        {
            present.entry(key.into()).or_insert(entry.received);
        }
    }
    Ok(reservations
        .into_iter()
        .map(|(key, total)| total.saturating_sub(*present.get(&key).unwrap_or(&0)))
        .sum())
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
    Ok(limit.saturating_sub(protected_used.saturating_add(reserved_missing_bytes(state).await?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn pins_are_resolved_without_an_active_profile_and_across_accounts() {
        let state = crate::cache::tests::test_state();
        let a = crate::cache::tests::insert_track(&state, 1, "123");
        crate::cache::tests::insert_track(&state, 2, "123");
        state.db.mark_audio_cache_pinned(a).unwrap();
        fs::create_dir_all(audio_dir(&state)).await.unwrap();
        fs::write(audio_dir(&state).join("123.mp3"), b"audio")
            .await
            .unwrap();
        assert!(list_audio_entries(&state).await.unwrap()[0].pinned);
        state.db.save_profile(2, "B", None, None, None).unwrap();
        assert!(list_audio_entries(&state).await.unwrap()[0].pinned);
        assert_eq!(
            list_audio_entries(&state).await.unwrap()[0].track_ids.len(),
            2
        );
        fs::remove_dir_all(&state.cache_dir).await.unwrap();
    }
    #[test]
    fn document_lease_protection_does_not_protect_neighboring_ids() {
        let protected = HashSet::from([PathBuf::from("/cache/123")]);
        assert!(path_is_protected(Path::new("/cache/123.mp3"), &protected));
        assert!(path_is_protected(
            Path::new("/cache/123.webm.part.meta"),
            &protected
        ));
        assert!(!path_is_protected(Path::new("/cache/1234.mp3"), &protected));
    }
    #[tokio::test]
    async fn orphan_cleanup_removes_corrupt_part_and_all_sidecars_but_retains_leases() {
        let state = crate::cache::tests::test_state();
        fs::create_dir_all(audio_dir(&state)).await.unwrap();
        for name in [
            "123.mp3.part",
            "123.mp3.part.meta",
            "123.mp3.part.meta.tmp",
            "124.complete",
            "125.mp3.part",
            "125.mp3.part.meta",
        ] {
            fs::write(audio_dir(&state).join(name), b"invalid")
                .await
                .unwrap();
        }
        let _lease = state.cache_leases.acquire("125");
        cleanup_orphans(&state, &protected_path_set(&state).await)
            .await
            .unwrap();
        for name in [
            "123.mp3.part",
            "123.mp3.part.meta",
            "123.mp3.part.meta.tmp",
            "124.complete",
        ] {
            assert!(!audio_dir(&state).join(name).exists());
        }
        assert!(audio_dir(&state).join("125.mp3.part").exists());
        fs::remove_dir_all(&state.cache_dir).await.unwrap();
    }
}
