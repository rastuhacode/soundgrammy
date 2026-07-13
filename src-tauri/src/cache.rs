//! Content-addressed on-disk cache for audio, thumbnails, and the avatar.
//!
//! Files are downloaded once (to a `.part` temp then atomically renamed) and
//! then served straight from disk through Tauri's asset protocol. Concurrent
//! plays of the same track are de-duplicated via per-key locks.

#![allow(deprecated)]

use std::path::{Path, PathBuf};

use grammers_tl_types as tl;
use tauri::AppHandle;

use crate::db::Track;
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

    tokio::fs::create_dir_all(audio_dir(state)).await?;
    let stream = state
        .streaming
        .start(app.clone(), track, dest.clone())
        .await?;
    stream.wait_complete().await
}

/// Ensures the track's thumbnail is cached (remote thumb first, then embedded
/// cover art from the cached audio) and returns its path, if any exists.
pub async fn ensure_thumbnail(
    state: &AppState,
    track_id: i64,
    high_quality: bool,
) -> AppResult<Option<PathBuf>> {
    let track = require_track(state, track_id)?;
    let suffix = if high_quality { ".full" } else { "" };
    let dest = thumb_dir(state).join(format!("{}{}.jpg", track.file_unique_id, suffix));
    if dest.exists() {
        return Ok(Some(dest));
    }

    tokio::fs::create_dir_all(thumb_dir(state)).await?;

    let key = format!("thumb:{}:{}", track.file_unique_id, high_quality);
    let lock = state.lock_for(&key).await;
    let _guard = lock.lock().await;

    if dest.exists() {
        return Ok(Some(dest));
    }

    if download::download_thumbnail(state, &track, &dest, high_quality).await? {
        return Ok(Some(dest));
    }

    // Fallback: pull embedded cover art from an already-cached audio file.
    let audio = audio_path(state, &track)?;
    if audio.exists() {
        if let Some(bytes) = extract_embedded_cover(&audio) {
            tokio::fs::write(&dest, &bytes).await?;
            return Ok(Some(dest));
        }
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
