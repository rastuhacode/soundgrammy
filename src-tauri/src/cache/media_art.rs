use std::path::{Path, PathBuf};

use ferogram::tl;

use super::{audio_path, require_track, thumb_dir};
use crate::db::Track;
use crate::error::AppResult;
use crate::state::AppState;
use crate::telegram::{auth, download};

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
    let client = state.client().await?;
    let user = auth::fetch_self_raw(&client).await?;
    let photo = match user.photo {
        Some(tl::enums::UserProfilePhoto::UserProfilePhoto(p)) => p,
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

    download::download_location(state, photo.dc_id, location, &dest).await?;
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
