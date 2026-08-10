//! Export cached (or freshly fetched) audio into the system Downloads folder.
//!
//! This is the user-facing "Download" action — distinct from app cache.
//! Playlist-level download (`download_playlist`) also writes an M3U8 here.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::cache;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::extension_for_mime;
use crate::telegram::download;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDownloadSucceeded {
    pub track_id: i64,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDownloadFailed {
    pub track_id: i64,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDownloadResult {
    /// Absolute folder path when at least one file was written; otherwise `None`.
    pub folder_path: Option<String>,
    pub succeeded: Vec<PlaylistDownloadSucceeded>,
    pub failed: Vec<PlaylistDownloadFailed>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaylistDownloadProgress {
    job_id: String,
    current: u32,
    total: u32,
    track_id: i64,
}

struct M3uEntry {
    duration_sec: i32,
    display: String,
    file_name: String,
}

fn sanitize_filename_part(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

fn track_export_basename(track: &crate::db::Track) -> String {
    let performer = track
        .performer
        .as_deref()
        .map(sanitize_filename_part)
        .filter(|s| !s.is_empty());
    let title = track
        .title
        .as_deref()
        .map(sanitize_filename_part)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Track {}", track.id));

    match performer {
        Some(artist) => format!("{artist} - {title}"),
        None => title,
    }
}

fn unique_path(dir: &Path, basename: &str, ext: &str) -> PathBuf {
    let primary = dir.join(format!("{basename}.{ext}"));
    if !primary.exists() {
        return primary;
    }
    let mut n = 2u32;
    loop {
        let candidate = dir.join(format!("{basename} ({n}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
        n = n.saturating_add(1);
        if n > 10_000 {
            return dir.join(format!("{basename} ({n}).{ext}"));
        }
    }
}

fn unique_dir(parent: &Path, basename: &str) -> PathBuf {
    let primary = parent.join(basename);
    if !primary.exists() {
        return primary;
    }
    let mut n = 2u32;
    loop {
        let candidate = parent.join(format!("{basename} ({n})"));
        if !candidate.exists() {
            return candidate;
        }
        n = n.saturating_add(1);
        if n > 10_000 {
            return parent.join(format!("{basename} ({n})"));
        }
    }
}

fn soundgrammy_downloads_root(app: &AppHandle) -> AppResult<PathBuf> {
    let downloads = app
        .path()
        .download_dir()
        .map_err(|e| AppError::msg(format!("Downloads folder unavailable: {e}")))?;
    Ok(downloads.join("SoundGrammy"))
}

fn local_date_ymd() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // YYYY-MM-DD in UTC (stable export-folder label without extra deps).
    let days = secs / 86_400;
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

async fn copy_track_to_dir(
    state: &AppState,
    app: &AppHandle,
    track_id: i64,
    dest_dir: &Path,
) -> AppResult<PathBuf> {
    let track = cache::require_track(state, track_id)?;
    let doc = download::stored_document(&track)?;
    let ext = extension_for_mime(&doc.mime_type);
    let basename = track_export_basename(&track);
    let dest = unique_path(dest_dir, &basename, ext);

    let cached = cache::ensure_audio(state, app, track_id).await?;
    tokio::fs::create_dir_all(dest_dir).await.map_err(|e| {
        AppError::msg(format!(
            "Cannot write to Downloads ({}): {e}",
            dest_dir.display()
        ))
    })?;
    tokio::fs::copy(&cached, &dest).await.map_err(|e| {
        AppError::msg(format!(
            "Cannot write download file ({}): {e}",
            dest.display()
        ))
    })?;
    Ok(dest)
}

/// Export a single track into `Downloads/SoundGrammy/`.
pub async fn export_track(state: &AppState, app: &AppHandle, track_id: i64) -> AppResult<String> {
    let root = soundgrammy_downloads_root(app)?;
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|e| AppError::msg(format!("Cannot create Downloads/SoundGrammy folder: {e}")))?;
    let path = copy_track_to_dir(state, app, track_id, &root).await?;
    Ok(path.to_string_lossy().into_owned())
}

/// Export many tracks into a dated subfolder under `Downloads/SoundGrammy/`.
/// Returns the export folder path.
pub async fn export_tracks(
    state: &AppState,
    app: &AppHandle,
    track_ids: &[i64],
) -> AppResult<String> {
    if track_ids.is_empty() {
        return Err(AppError::msg("No tracks to download"));
    }
    let root = soundgrammy_downloads_root(app)?;
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|e| AppError::msg(format!("Cannot create Downloads/SoundGrammy folder: {e}")))?;
    let folder_name = format!("Export {}", local_date_ymd());
    let dest_dir = unique_dir(&root, &folder_name);
    tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| {
        AppError::msg(format!(
            "Cannot create export folder ({}): {e}",
            dest_dir.display()
        ))
    })?;

    for &track_id in track_ids {
        copy_track_to_dir(state, app, track_id, &dest_dir).await?;
    }

    Ok(dest_dir.to_string_lossy().into_owned())
}

fn playlist_folder_basename(name: &str) -> String {
    let sanitized = sanitize_filename_part(name);
    if sanitized.is_empty() {
        "Playlist".to_string()
    } else {
        sanitized
    }
}

fn track_display_label(track: &crate::db::Track) -> String {
    let title = track
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown title");
    match track
        .performer
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(artist) => format!("{artist} - {title}"),
        None => title.to_string(),
    }
}

fn extinf_duration(duration: Option<i64>) -> i32 {
    match duration {
        Some(d) if d >= 0 && d <= i32::MAX as i64 => d as i32,
        _ => -1,
    }
}

fn build_m3u8(entries: &[M3uEntry]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for entry in entries {
        out.push_str(&format!(
            "#EXTINF:{},{}\n{}\n",
            entry.duration_sec, entry.display, entry.file_name
        ));
    }
    out
}

/// Download an ordered playlist into `Downloads/SoundGrammy/<name>/` with an M3U8.
/// Continues on per-track failures (partial success). Does not use the bulk cache size gate.
pub async fn download_playlist(
    state: &AppState,
    app: &AppHandle,
    name: String,
    track_ids: &[i64],
    job_id: String,
) -> AppResult<PlaylistDownloadResult> {
    if track_ids.is_empty() {
        return Err(AppError::msg("No tracks to download"));
    }

    let root = soundgrammy_downloads_root(app)?;
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|e| AppError::msg(format!("Cannot create Downloads/SoundGrammy folder: {e}")))?;

    let folder_basename = playlist_folder_basename(&name);
    let dest_dir = unique_dir(&root, &folder_basename);
    tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| {
        AppError::msg(format!(
            "Cannot create playlist folder ({}): {e}",
            dest_dir.display()
        ))
    })?;

    let total = track_ids.len() as u32;
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    let mut m3u_entries = Vec::new();

    for (index, &track_id) in track_ids.iter().enumerate() {
        let current = (index as u32).saturating_add(1);
        let _ = app.emit(
            "download_playlist:progress",
            PlaylistDownloadProgress {
                job_id: job_id.clone(),
                current,
                total,
                track_id,
            },
        );

        let meta = cache::require_track(state, track_id).ok();
        let title = meta.as_ref().and_then(|t| t.title.clone());
        let performer = meta.as_ref().and_then(|t| t.performer.clone());
        let duration = meta.as_ref().and_then(|t| t.duration);
        let display = meta
            .as_ref()
            .map(track_display_label)
            .unwrap_or_else(|| format!("Track {track_id}"));

        match copy_track_to_dir(state, app, track_id, &dest_dir).await {
            Ok(path) => {
                let file_name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                m3u_entries.push(M3uEntry {
                    duration_sec: extinf_duration(duration),
                    display,
                    file_name: file_name.clone(),
                });
                succeeded.push(PlaylistDownloadSucceeded {
                    track_id,
                    title,
                    performer,
                    file_name,
                });
            }
            Err(err) => {
                failed.push(PlaylistDownloadFailed {
                    track_id,
                    title,
                    performer,
                    error: err.to_string(),
                });
            }
        }
    }

    if succeeded.is_empty() {
        let _ = tokio::fs::remove_dir_all(&dest_dir).await;
        return Ok(PlaylistDownloadResult {
            folder_path: None,
            succeeded,
            failed,
        });
    }

    let m3u_name = format!("{folder_basename}.m3u8");
    let m3u_path = dest_dir.join(&m3u_name);
    let m3u_body = build_m3u8(&m3u_entries);
    // Audio files are already on disk — do not fail the whole job if the
    // playlist file cannot be written; the UI needs the per-track summary.
    let _ = tokio::fs::write(&m3u_path, m3u_body.as_bytes()).await;

    Ok(PlaylistDownloadResult {
        folder_path: Some(dest_dir.to_string_lossy().into_owned()),
        succeeded,
        failed,
    })
}

#[cfg(test)]
mod tests;
