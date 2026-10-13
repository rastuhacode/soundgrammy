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
    pub warnings: Vec<String>,
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

#[cfg(test)]
fn unique_path(dir: &Path, basename: &str, ext: &str) -> PathBuf {
    for n in 1..=10_000 {
        let name = if n == 1 {
            format!("{basename}.{ext}")
        } else {
            format!("{basename} ({n}).{ext}")
        };
        let path = dir.join(name);
        if !path.exists() {
            return path;
        }
    }
    panic!("No unused export filename");
}

fn unique_dir(parent: &Path, basename: &str) -> std::io::Result<PathBuf> {
    for n in 1..=10_000 {
        let name = if n == 1 {
            basename.to_string()
        } else {
            format!("{basename} ({n})")
        };
        let path = parent.join(name);
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::other("No unused export directory"))
}

async fn copy_complete_file(
    source: &Path,
    dir: &Path,
    basename: &str,
    ext: &str,
) -> AppResult<PathBuf> {
    use tokio::io::AsyncWriteExt;
    tokio::fs::create_dir_all(dir).await?;
    let temp = dir.join(format!(".soundgrammy-{:016x}.tmp", rand::random::<u64>()));
    let result = async {
        let mut input = tokio::fs::File::open(source).await?;
        let mut output = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .await?;
        tokio::io::copy(&mut input, &mut output).await?;
        output.flush().await?;
        output.sync_all().await?;
        drop(output);
        // Linking the completed inode publishes atomically and cannot overwrite another export.
        for n in 1..=10_000 {
            let name = if n == 1 {
                format!("{basename}.{ext}")
            } else {
                format!("{basename} ({n}).{ext}")
            };
            let destination = dir.join(name);
            match tokio::fs::hard_link(&temp, &destination).await {
                Ok(()) => return Ok(destination),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(AppError::Io(error)),
            }
        }
        Err(AppError::msg("No unused export filename"))
    }
    .await;
    let _ = tokio::fs::remove_file(temp).await;
    result
}

fn soundgrammy_downloads_root(app: &AppHandle) -> AppResult<PathBuf> {
    let downloads = app
        .path()
        .download_dir()
        .map_err(|e| AppError::msg(format!("Downloads folder unavailable: {e}")))?;
    Ok(downloads.join("SoundGrammy"))
}

fn utc_date_ymd() -> String {
    crate::time::date_utc(crate::time::now_ms())
}

fn finalized_format(path: &Path, track: &crate::db::Track) -> AppResult<(String, String)> {
    let ext = path.extension().and_then(|s| s.to_str());
    let mime = match ext {
        Some("webm") => "audio/webm",
        Some("ogg") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("m4a") => "audio/mp4",
        Some("flac") => "audio/flac",
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        _ => {
            return Ok((
                extension_for_mime(&download::stored_document(track)?.mime_type).into(),
                download::stored_document(track)?.mime_type,
            ))
        }
    };
    Ok((ext.unwrap().into(), mime.into()))
}

async fn copy_track_to_dir(
    state: &AppState,
    app: &AppHandle,
    track_id: i64,
    dest_dir: &Path,
) -> AppResult<PathBuf> {
    let track = cache::require_track(state, track_id)?;
    let cached = cache::ensure_audio_leased(state, app, track_id).await?;
    let (ext, _) = finalized_format(&cached, &track)?;
    let basename = track_export_basename(&track);
    // Revalidate ownership after a download that may outlive logout/account replacement.
    cache::require_track(state, track_id)?;
    copy_complete_file(&cached, dest_dir, &basename, &ext).await
}

/// Export a single track into `Downloads/SoundGrammy/`.
pub async fn export_track(state: &AppState, app: &AppHandle, track_id: i64) -> AppResult<String> {
    #[cfg(target_os = "android")]
    {
        let track = cache::require_track(state, track_id)?;
        let cached = cache::ensure_audio_leased(state, app, track_id).await?;
        let (ext, mime) = finalized_format(&cached, &track)?;
        let file_name = format!("{}.{}", track_export_basename(&track), ext);
        return crate::android_downloads::save(cached.path.clone(), file_name, mime).await;
    }

    #[cfg(not(target_os = "android"))]
    {
        let root = soundgrammy_downloads_root(app)?;
        tokio::fs::create_dir_all(&root).await.map_err(|e| {
            AppError::msg(format!("Cannot create Downloads/SoundGrammy folder: {e}"))
        })?;
        let path = copy_track_to_dir(state, app, track_id, &root).await?;
        Ok(path.to_string_lossy().into_owned())
    }
}

/// Export many tracks into a dated subfolder under `Downloads/SoundGrammy/`.
/// Returns the export folder path.
pub async fn export_tracks(
    state: &AppState,
    app: &AppHandle,
    track_ids: &[i64],
) -> AppResult<PlaylistDownloadResult> {
    export_folder(
        state,
        app,
        format!("Export {}", utc_date_ymd()),
        track_ids,
        None,
        false,
    )
    .await
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
    export_folder(state, app, name, track_ids, Some(job_id), true).await
}

async fn export_folder(
    state: &AppState,
    app: &AppHandle,
    name: String,
    track_ids: &[i64],
    job_id: Option<String>,
    playlist: bool,
) -> AppResult<PlaylistDownloadResult> {
    if track_ids.is_empty() {
        return Err(AppError::msg("No tracks to download"));
    }

    let root = soundgrammy_downloads_root(app)?;
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|e| AppError::msg(format!("Cannot create Downloads/SoundGrammy folder: {e}")))?;

    let folder_basename = playlist_folder_basename(&name);
    let dest_dir = unique_dir(&root, &folder_basename)?;
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
        if let Some(job_id) = &job_id {
            let _ = app.emit(
                "download_playlist:progress",
                PlaylistDownloadProgress {
                    job_id: job_id.clone(),
                    current,
                    total,
                    track_id,
                },
            );
        }

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

    finish_export(
        dest_dir,
        folder_basename,
        succeeded,
        failed,
        m3u_entries,
        playlist,
        |path, body| async move {
            tokio::task::spawn_blocking(move || crate::atomic_file::write(&path, body.as_bytes()))
                .await
                .map_err(|_| AppError::msg("Playlist file worker interrupted"))??;
            Ok(())
        },
    )
    .await
}

async fn finish_export<F: std::future::Future<Output = AppResult<()>>>(
    dest_dir: PathBuf,
    folder_basename: String,
    succeeded: Vec<PlaylistDownloadSucceeded>,
    failed: Vec<PlaylistDownloadFailed>,
    m3u_entries: Vec<M3uEntry>,
    playlist: bool,
    write_playlist: impl FnOnce(PathBuf, String) -> F,
) -> AppResult<PlaylistDownloadResult> {
    if succeeded.is_empty() {
        let _ = tokio::fs::remove_dir_all(&dest_dir).await;
        return Ok(PlaylistDownloadResult {
            folder_path: None,
            succeeded,
            failed,
            warnings: Vec::new(),
        });
    }

    let mut warnings = Vec::new();
    if playlist {
        let m3u_path = dest_dir.join(format!("{folder_basename}.m3u8"));
        let m3u_body = build_m3u8(&m3u_entries);
        if write_playlist(m3u_path, m3u_body).await.is_err() {
            warnings.push(
                "Audio files were exported, but the playlist file could not be written.".into(),
            );
        }
    }

    Ok(PlaylistDownloadResult {
        folder_path: Some(dest_dir.to_string_lossy().into_owned()),
        succeeded,
        failed,
        warnings,
    })
}

#[cfg(test)]
mod tests;
