//! JSON playlist recipe export/import (same-account cross-device sync).
//!
//! Distinct from Downloads folder export (`export.rs` / `download_playlist`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::db::Track;
use crate::error::{AppError, AppResult};

const RECIPE_FORMAT: &str = "soundgrammy.playlist";
const RECIPE_VERSION: u32 = 1;

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

fn require_uid(db: &Db) -> AppResult<i64> {
    db.active_uid()
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
    crate::time::iso_utc(crate::time::now_ms())
}

fn build_recipe(
    db: &Db,
    uid: i64,
    source: &PlaylistRecipeSource,
) -> AppResult<PlaylistRecipeDocument> {
    let (name, kind, playlist_id) = match source {
        PlaylistRecipeSource::Liked => {
            let id = db.liked_playlist_id(uid)?;
            ("Liked".to_string(), "liked".to_string(), id)
        }
        PlaylistRecipeSource::Custom { playlist_id } => {
            let (name, kind) = db.custom_playlist_name_and_kind(*playlist_id, uid)?;
            if kind != "custom" {
                return Err(AppError::msg(
                    "Only Liked and custom playlists can be exported as JSON",
                ));
            }
            (name, kind, *playlist_id)
        }
    };

    let tracks = db
        .playlist_tracks_ordered(playlist_id, uid)?
        .iter()
        .map(track_to_recipe)
        .collect();

    Ok(PlaylistRecipeDocument {
        format: RECIPE_FORMAT.to_string(),
        version: RECIPE_VERSION,
        exported_at: iso_now(),
        exporter: PlaylistRecipeExporter { tg_user_id: uid },
        playlist: PlaylistRecipeBody { name, kind, tracks },
    })
}

pub fn export_playlist_json(db: &Db, source: PlaylistRecipeSource, path: String) -> AppResult<()> {
    let uid = require_uid(db)?;
    let recipe = build_recipe(db, uid, &source)?;
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
    if recipe.playlist.tracks.is_empty() {
        return Err(AppError::msg("Playlist file contains no tracks"));
    }
    Ok(recipe)
}

fn match_recipe_tracks(
    db: &Db,
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
    let resolved = db.track_ids_by_file_unique_ids(uid, &unique_ids)?;

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

fn load_recipe_from_path(db: &Db, path: &str) -> AppResult<(i64, PlaylistRecipeDocument)> {
    let uid = require_uid(db)?;
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::msg(format!("Cannot read playlist file: {e}")))?;
    let recipe = parse_and_validate_recipe(&raw, uid)?;
    Ok((uid, recipe))
}

/// Analyze a recipe file without creating a playlist (prepare / preview phase).
pub fn analyze_playlist_json(db: &Db, path: String) -> AppResult<PlaylistImportPreview> {
    let (uid, recipe) = load_recipe_from_path(db, &path)?;
    let (_matched_ids, succeeded, failed) = match_recipe_tracks(db, uid, &recipe)?;
    Ok(PlaylistImportPreview {
        suggested_name: recipe.playlist.name.trim().to_string(),
        succeeded,
        failed,
    })
}

/// Create a custom playlist from a recipe. `name` overrides the recipe title when set.
pub fn import_playlist_json(
    db: &Db,
    path: String,
    name: Option<String>,
) -> AppResult<PlaylistImportResult> {
    let (uid, recipe) = load_recipe_from_path(db, &path)?;
    let (matched_ids, succeeded, failed) = match_recipe_tracks(db, uid, &recipe)?;

    let playlist_name = name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| recipe.playlist.name.trim());
    if playlist_name.is_empty() {
        return Err(AppError::msg("Playlist name is required"));
    }
    if playlist_name.chars().count() > 100 {
        return Err(AppError::msg(
            "Playlist name must be at most 100 characters",
        ));
    }

    let created = db.create_playlist_with_tracks(uid, playlist_name, &matched_ids)?;

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
    fn recipe_round_trips_json() {
        let recipe = sample_recipe(42);
        let json = serde_json::to_string(&recipe).unwrap();
        let parsed: PlaylistRecipeDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.format, RECIPE_FORMAT);
        assert_eq!(parsed.exporter.tg_user_id, 42);
        assert_eq!(parsed.playlist.name, "Gym");
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
    fn validate_rejects_empty_tracks() {
        let mut recipe = sample_recipe(1);
        recipe.playlist.tracks.clear();
        let json = serde_json::to_string(&recipe).unwrap();
        let err = parse_and_validate_recipe(&json, 1).unwrap_err();
        assert!(err.to_string().contains("no tracks"));
    }
    #[test]
    fn recipe_membership_failure_rolls_back_playlist_and_preserves_duplicate_order_on_retry() {
        let state = crate::cache::tests::test_state();
        state.db.save_profile(1, "User", None, None, None).unwrap();
        let track = crate::cache::tests::insert_track(&state, 1, "123");
        let mut recipe = sample_recipe(1);
        recipe.playlist.tracks[0].file_unique_id = "123".into();
        recipe
            .playlist
            .tracks
            .push(recipe.playlist.tracks[0].clone());
        std::fs::create_dir_all(&state.data_dir).unwrap();
        let path = state.data_dir.join("recipe.json");
        std::fs::write(&path, serde_json::to_vec(&recipe).unwrap()).unwrap();
        state.db.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_recipe BEFORE INSERT ON playlist_tracks BEGIN SELECT RAISE(ABORT,'injected membership failure'); END;").unwrap();
        assert!(
            import_playlist_json(&state.db, path.to_string_lossy().into_owned(), None).is_err()
        );
        assert!(state.db.playlists_bundle(1).unwrap().custom.is_empty());
        state
            .db
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_recipe")
            .unwrap();
        let imported =
            import_playlist_json(&state.db, path.to_string_lossy().into_owned(), None).unwrap();
        assert_eq!(
            state
                .db
                .playlists_bundle(1)
                .unwrap()
                .custom
                .iter()
                .find(|p| p.id == imported.playlist_id)
                .unwrap()
                .track_ids,
            vec![track, track]
        );
        std::fs::remove_dir_all(&state.data_dir).unwrap();
    }
}
