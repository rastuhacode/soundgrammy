//! JSON playlist recipe export/import (same-account cross-device sync).
//!
//! Distinct from Downloads folder export (`export.rs` / `download_playlist`).

use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const RECIPE_FORMAT: &str = "soundgrammy.playlist";
const RECIPE_VERSION: u32 = 1;
const MAX_THUMBNAIL_BYTES: usize = 512 * 1024;
const ALLOWED_THUMB_MIMES: &[&str] = &["image/jpeg", "image/png", "image/webp"];

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlaylistRecipeSource {
    Liked,
    Custom {
        #[serde(alias = "playlistId")]
        playlist_id: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRecipeThumbnail {
    pub mime: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRecipeTrack {
    pub file_unique_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRecipeBody {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PlaylistRecipeThumbnail>,
    pub tracks: Vec<PlaylistRecipeTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRecipeExporter {
    pub tg_user_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRecipeDocument {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub exporter: PlaylistRecipeExporter,
    pub playlist: PlaylistRecipeBody,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistImportSucceeded {
    pub file_unique_id: String,
    pub title: Option<String>,
    pub performer: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistImportFailed {
    pub file_unique_id: String,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistImportPreview {
    pub suggested_name: String,
    pub succeeded: Vec<PlaylistImportSucceeded>,
    pub failed: Vec<PlaylistImportFailed>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistImportResult {
    pub playlist_id: i64,
    pub playlist_name: String,
    pub succeeded: Vec<PlaylistImportSucceeded>,
    pub failed: Vec<PlaylistImportFailed>,
}

fn require_uid(state: &AppState) -> AppResult<i64> {
    state
        .db
        .load_profile()?
        .map(|p| p.tg_user_id)
        .ok_or(AppError::NotAuthorized)
}

fn track_to_recipe(track: &Track) -> PlaylistRecipeTrack {
    PlaylistRecipeTrack {
        file_unique_id: track.file_unique_id.clone(),
        title: track.title.clone(),
        performer: track.performer.clone(),
        duration_sec: track.duration,
    }
}

fn iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Stable ISO-8601 UTC without chrono dep (second precision).
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if month <= 2 { y + 1 } else { y };
    format!("{y:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn validate_thumbnail(thumb: &PlaylistRecipeThumbnail) -> AppResult<()> {
    let mime = thumb.mime.trim();
    if !ALLOWED_THUMB_MIMES.contains(&mime) {
        return Err(AppError::msg(
            "Playlist thumbnail must be JPEG, PNG, or WebP",
        ));
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(thumb.data_base64.trim())
        .map_err(|_| AppError::msg("Playlist thumbnail data is not valid base64"))?;
    if decoded.is_empty() {
        return Err(AppError::msg("Playlist thumbnail is empty"));
    }
    if decoded.len() > MAX_THUMBNAIL_BYTES {
        return Err(AppError::msg(
            "Playlist thumbnail must be smaller than 512KB",
        ));
    }
    Ok(())
}

fn build_recipe(
    state: &AppState,
    uid: i64,
    source: &PlaylistRecipeSource,
) -> AppResult<PlaylistRecipeDocument> {
    let (name, kind, playlist_id, thumbnail) = match source {
        PlaylistRecipeSource::Liked => {
            let id = state.db.liked_playlist_id(uid)?;
            ("Liked".to_string(), "liked".to_string(), id, None)
        }
        PlaylistRecipeSource::Custom { playlist_id } => {
            let (name, kind) = state.db.custom_playlist_name_and_kind(*playlist_id, uid)?;
            if kind != "custom" {
                return Err(AppError::msg(
                    "Only Liked and custom playlists can be exported as JSON",
                ));
            }
            let thumb = state
                .db
                .playlist_thumbnail(*playlist_id, uid)?
                .map(|(data, mime)| PlaylistRecipeThumbnail {
                    mime,
                    data_base64: data,
                });
            (name, kind, *playlist_id, thumb)
        }
    };

    let tracks = state
        .db
        .playlist_tracks_ordered(playlist_id, uid)?
        .iter()
        .map(track_to_recipe)
        .collect();

    Ok(PlaylistRecipeDocument {
        format: RECIPE_FORMAT.to_string(),
        version: RECIPE_VERSION,
        exported_at: iso_now(),
        exporter: PlaylistRecipeExporter { tg_user_id: uid },
        playlist: PlaylistRecipeBody {
            name,
            kind,
            thumbnail,
            tracks,
        },
    })
}

pub fn export_playlist_json(
    state: &AppState,
    source: PlaylistRecipeSource,
    path: String,
) -> AppResult<()> {
    let uid = require_uid(state)?;
    let recipe = build_recipe(state, uid, &source)?;
    let json = serde_json::to_string_pretty(&recipe)?;
    let dest = Path::new(&path);
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::msg(format!("Cannot create export folder: {e}")))?;
        }
    }
    std::fs::write(dest, json.as_bytes()).map_err(|e| {
        AppError::msg(format!(
            "Cannot write playlist file ({}): {e}",
            dest.display()
        ))
    })?;
    Ok(())
}

fn parse_and_validate_recipe(raw: &str, uid: i64) -> AppResult<PlaylistRecipeDocument> {
    let recipe: PlaylistRecipeDocument = serde_json::from_str(raw)
        .map_err(|e| AppError::msg(format!("Invalid playlist file: {e}")))?;
    if recipe.format != RECIPE_FORMAT {
        return Err(AppError::msg(format!(
            "Unsupported playlist format (expected {RECIPE_FORMAT})"
        )));
    }
    if recipe.version != RECIPE_VERSION {
        return Err(AppError::msg(format!(
            "Unsupported playlist version (expected {RECIPE_VERSION})"
        )));
    }
    if recipe.exporter.tg_user_id != uid {
        return Err(AppError::msg(
            "This playlist was exported from another Telegram account and cannot be imported",
        ));
    }
    let name = recipe.playlist.name.trim();
    if name.is_empty() {
        return Err(AppError::msg("Playlist name is required"));
    }
    if let Some(ref thumb) = recipe.playlist.thumbnail {
        validate_thumbnail(thumb)?;
    }
    if recipe.playlist.tracks.is_empty() {
        return Err(AppError::msg("Playlist file contains no tracks"));
    }
    Ok(recipe)
}

fn match_recipe_tracks(
    state: &AppState,
    uid: i64,
    recipe: &PlaylistRecipeDocument,
) -> AppResult<(
    Vec<i64>,
    Vec<PlaylistImportSucceeded>,
    Vec<PlaylistImportFailed>,
)> {
    let unique_ids: Vec<String> = recipe
        .playlist
        .tracks
        .iter()
        .map(|t| t.file_unique_id.clone())
        .collect();
    let resolved = state.db.track_ids_by_file_unique_ids(uid, &unique_ids)?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    let mut matched_ids = Vec::new();

    for track in &recipe.playlist.tracks {
        match resolved.get(&track.file_unique_id) {
            Some(&id) => {
                matched_ids.push(id);
                succeeded.push(PlaylistImportSucceeded {
                    file_unique_id: track.file_unique_id.clone(),
                    title: track.title.clone(),
                    performer: track.performer.clone(),
                });
            }
            None => {
                failed.push(PlaylistImportFailed {
                    file_unique_id: track.file_unique_id.clone(),
                    title: track.title.clone(),
                    performer: track.performer.clone(),
                    reason: "notInLibrary".into(),
                });
            }
        }
    }

    if matched_ids.is_empty() {
        return Err(AppError::msg(
            "None of the tracks in this playlist are in your library. Sync Saved Music and try again.",
        ));
    }

    Ok((matched_ids, succeeded, failed))
}

fn load_recipe_from_path(state: &AppState, path: &str) -> AppResult<(i64, PlaylistRecipeDocument)> {
    let uid = require_uid(state)?;
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::msg(format!("Cannot read playlist file: {e}")))?;
    let recipe = parse_and_validate_recipe(&raw, uid)?;
    Ok((uid, recipe))
}

/// Analyze a recipe file without creating a playlist (prepare / preview phase).
pub fn analyze_playlist_json(state: &AppState, path: String) -> AppResult<PlaylistImportPreview> {
    let (uid, recipe) = load_recipe_from_path(state, &path)?;
    let (_matched_ids, succeeded, failed) = match_recipe_tracks(state, uid, &recipe)?;
    Ok(PlaylistImportPreview {
        suggested_name: recipe.playlist.name.trim().to_string(),
        succeeded,
        failed,
    })
}

/// Create a custom playlist from a recipe. `name` overrides the recipe title when set.
pub fn import_playlist_json(
    state: &AppState,
    path: String,
    name: Option<String>,
) -> AppResult<PlaylistImportResult> {
    let (uid, recipe) = load_recipe_from_path(state, &path)?;
    let (matched_ids, succeeded, failed) = match_recipe_tracks(state, uid, &recipe)?;

    let playlist_name = name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| recipe.playlist.name.trim());
    if playlist_name.is_empty() {
        return Err(AppError::msg("Playlist name is required"));
    }
    if playlist_name.len() > 100 {
        return Err(AppError::msg(
            "Playlist name must be at most 100 characters",
        ));
    }

    let thumb_owned = recipe
        .playlist
        .thumbnail
        .as_ref()
        .map(|t| (t.data_base64.trim().to_string(), t.mime.trim().to_string()));
    let created = state.db.create_playlist(
        uid,
        playlist_name,
        thumb_owned.as_ref().map(|(d, m)| (d.as_str(), m.as_str())),
    )?;
    state
        .db
        .add_tracks_to_playlist(created.id, &matched_ids, uid)?;

    Ok(PlaylistImportResult {
        playlist_id: created.id,
        playlist_name: created.name,
        succeeded,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recipe(uid: i64) -> PlaylistRecipeDocument {
        PlaylistRecipeDocument {
            format: RECIPE_FORMAT.into(),
            version: RECIPE_VERSION,
            exported_at: "2026-07-27T12:00:00Z".into(),
            exporter: PlaylistRecipeExporter { tg_user_id: uid },
            playlist: PlaylistRecipeBody {
                name: "Gym".into(),
                kind: "custom".into(),
                thumbnail: Some(PlaylistRecipeThumbnail {
                    mime: "image/jpeg".into(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(b"fake-jpeg"),
                }),
                tracks: vec![PlaylistRecipeTrack {
                    file_unique_id: "doc1".into(),
                    title: Some("Bangarang".into()),
                    performer: Some("Skrillex".into()),
                    duration_sec: Some(215),
                }],
            },
        }
    }

    #[test]
    fn recipe_round_trips_json_with_thumbnail() {
        let recipe = sample_recipe(42);
        let json = serde_json::to_string(&recipe).unwrap();
        let parsed: PlaylistRecipeDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.format, RECIPE_FORMAT);
        assert_eq!(parsed.exporter.tg_user_id, 42);
        assert_eq!(parsed.playlist.name, "Gym");
        assert!(parsed.playlist.thumbnail.is_some());
        assert_eq!(parsed.playlist.tracks[0].file_unique_id, "doc1");
    }

    #[test]
    fn validate_rejects_wrong_account() {
        let recipe = sample_recipe(1);
        let json = serde_json::to_string(&recipe).unwrap();
        let err = parse_and_validate_recipe(&json, 2).unwrap_err();
        assert!(err.to_string().contains("another Telegram account"));
    }

    #[test]
    fn validate_rejects_bad_mime() {
        let mut recipe = sample_recipe(1);
        recipe.playlist.thumbnail = Some(PlaylistRecipeThumbnail {
            mime: "image/gif".into(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(b"x"),
        });
        let json = serde_json::to_string(&recipe).unwrap();
        let err = parse_and_validate_recipe(&json, 1).unwrap_err();
        assert!(err.to_string().contains("JPEG"));
    }

    #[test]
    fn validate_rejects_empty_tracks() {
        let mut recipe = sample_recipe(1);
        recipe.playlist.tracks.clear();
        let json = serde_json::to_string(&recipe).unwrap();
        let err = parse_and_validate_recipe(&json, 1).unwrap_err();
        assert!(err.to_string().contains("no tracks"));
    }
}
