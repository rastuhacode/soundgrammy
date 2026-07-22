//! Export cached (or freshly fetched) audio into the system Downloads folder.
//!
//! This is the user-facing "Download" action — distinct from app cache.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::cache;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::extension_for_mime;
use crate::telegram::download;

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
pub async fn export_track(
    state: &AppState,
    app: &AppHandle,
    track_id: i64,
) -> AppResult<String> {
    let root = soundgrammy_downloads_root(app)?;
    tokio::fs::create_dir_all(&root).await.map_err(|e| {
        AppError::msg(format!("Cannot create Downloads/SoundGrammy folder: {e}"))
    })?;
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
    tokio::fs::create_dir_all(&root).await.map_err(|e| {
        AppError::msg(format!("Cannot create Downloads/SoundGrammy folder: {e}"))
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Track;
    use std::fs;

    fn sample_track(
        id: i64,
        title: Option<&str>,
        performer: Option<&str>,
    ) -> Track {
        Track {
            id,
            tg_user_id: 1,
            file_id: format!("f{id}"),
            file_unique_id: format!("u{id}"),
            title: title.map(str::to_string),
            performer: performer.map(str::to_string),
            duration: Some(120),
            source: "mtproto".into(),
            mime_type: Some("audio/mpeg".into()),
            file_size: Some(1024),
            created_at: "2024-01-01T00:00:00Z".into(),
            mtproto_document: None,
        }
    }

    #[test]
    fn sanitize_replaces_forbidden_filename_chars() {
        assert_eq!(sanitize_filename_part("a/b:c*d?e\"f<g>h|i"), "a_b_c_d_e_f_g_h_i");
        assert_eq!(sanitize_filename_part("  spaced  "), "spaced");
        assert_eq!(sanitize_filename_part("...dots..."), "dots");
        assert_eq!(sanitize_filename_part("   "), "");
    }

    #[test]
    fn export_basename_prefers_artist_title() {
        let track = sample_track(7, Some("Bangarang"), Some("Skrillex"));
        assert_eq!(track_export_basename(&track), "Skrillex - Bangarang");
    }

    #[test]
    fn export_basename_falls_back_when_metadata_missing() {
        let no_artist = sample_track(3, Some("Solo"), None);
        assert_eq!(track_export_basename(&no_artist), "Solo");

        let empty = sample_track(9, None, None);
        assert_eq!(track_export_basename(&empty), "Track 9");

        let unsafe_chars = sample_track(1, Some("a/b"), Some("x:y"));
        assert_eq!(track_export_basename(&unsafe_chars), "x_y - a_b");
    }

    #[test]
    fn unique_path_appends_numeric_suffix_on_collision() {
        let dir = std::env::temp_dir().join(format!(
            "soundgrammy-export-path-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let first = unique_path(&dir, "Song", "mp3");
        assert_eq!(first.file_name().unwrap(), "Song.mp3");
        fs::write(&first, b"a").unwrap();

        let second = unique_path(&dir, "Song", "mp3");
        assert_eq!(second.file_name().unwrap(), "Song (2).mp3");
        fs::write(&second, b"b").unwrap();

        let third = unique_path(&dir, "Song", "mp3");
        assert_eq!(third.file_name().unwrap(), "Song (3).mp3");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_dir_appends_numeric_suffix_on_collision() {
        let parent = std::env::temp_dir().join(format!(
            "soundgrammy-export-dir-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();

        let first = unique_dir(&parent, "Export 2026-07-22");
        fs::create_dir_all(&first).unwrap();
        let second = unique_dir(&parent, "Export 2026-07-22");
        assert_eq!(
            second.file_name().unwrap(),
            "Export 2026-07-22 (2)"
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn local_date_ymd_is_iso_shaped() {
        let value = local_date_ymd();
        assert!(
            regex_is_ymd(&value),
            "expected YYYY-MM-DD, got {value}"
        );
    }

    fn regex_is_ymd(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[..4].iter().all(u8::is_ascii_digit)
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[8..].iter().all(u8::is_ascii_digit)
    }
}
