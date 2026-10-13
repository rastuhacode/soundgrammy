use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use ferogram::tl;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{audio_path, require_track, thumb_dir};
use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::{auth, download};

/// Ensures the track's thumbnail is cached and returns its path, if any exists.
///
/// Standard quality prefers the remote Telegram thumb, then embedded cover from
/// a cached audio file. High quality (fullscreen) prefers embedded cover when
/// the audio is already cached (`{id}.embed.jpg`), then falls back to the remote
/// thumb (`{id}.full.jpg`). High-quality IPC requests also schedule a bounded
/// leading-ID3 probe independently, notifying visible consumers on completion.
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
        if let Some(bytes) = tokio::task::spawn_blocking(move || extract_embedded_cover(&audio))
            .await
            .map_err(|error| AppError::msg(error.to_string()))?
        {
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
        if let Some(bytes) = tokio::task::spawn_blocking(move || extract_embedded_cover(&audio))
            .await
            .map_err(|error| AppError::msg(error.to_string()))?
        {
            write_embedded_cover(state, &embed_dest, &bytes).await?;
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

const MAX_ID3_BYTES: u64 = 8 * 1024 * 1024;
const ARTWORK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtworkChanged {
    track_id: i64,
    tg_user_id: i64,
    path: String,
}

/// The job owns its chunk futures until they settle. Dropping a timeout future
/// mid-transfer would strand a shared chunk in Loading and stall playback.
pub async fn schedule_embedded_artwork(
    state: &AppState,
    app: &AppHandle,
    track_id: i64,
) -> AppResult<()> {
    let track = require_track(state, track_id)?;
    let thumbs = thumb_dir(state);
    let embed = thumbs.join(format!("{}.embed.jpg", track.file_unique_id));
    let absent = thumbs.join(format!("{}.id3-none", track.file_unique_id));
    if embed.exists() || absent.exists() || audio_path(state, &track)?.exists() {
        return Ok(());
    }
    let lock = state
        .lock_for(&format!("artwork:{}", track.file_unique_id))
        .await;
    let Ok(guard) = lock.try_lock_owned() else {
        return Ok(());
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let state = app.state::<AppState>();
        if embed.exists() || absent.exists() || !artwork_account_current(&state, &track) {
            return;
        }
        let active = Arc::new(AtomicBool::new(true));
        let cancel = active.clone();
        let watchdog_app = app.clone();
        let owner = track.tg_user_id;
        let watchdog = tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + ARTWORK_TIMEOUT;
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let state = watchdog_app.state::<AppState>();
                if tokio::time::Instant::now() >= deadline
                    || super::current_uid(&state).ok() != Some(owner)
                {
                    cancel.store(false, Ordering::Release);
                    break;
                }
            }
        });
        let result = probe_embedded_artwork(&state, &app, &track, active.clone()).await;
        watchdog.abort();
        if !active.load(Ordering::Acquire) || !artwork_account_current(&state, &track) {
            return;
        }
        match result {
            Ok(Some(bytes)) => {
                if !artwork_account_current(&state, &track) {
                    return;
                }
                if let Err(error) = write_embedded_cover(&state, &embed, &bytes).await {
                    tracing::debug!(track_id, %error, "Artwork cache write failed");
                    return;
                }
                if artwork_account_current(&state, &track) {
                    let _ = app.emit(
                        "artwork:changed",
                        ArtworkChanged {
                            track_id,
                            tg_user_id: track.tg_user_id,
                            path: embed.to_string_lossy().into_owned(),
                        },
                    );
                }
            }
            Ok(None) => {
                // Only a successful probe can memoize absence. Whole-file
                // extraction still runs later for other containers/tag layouts.
                let _ = tokio::fs::create_dir_all(&thumbs).await;
                let _ = tokio::fs::write(absent, []).await;
            }
            Err(error) => tracing::debug!(track_id, %error, "Embedded artwork probe deferred"),
        }
    });
    Ok(())
}

fn artwork_account_current(state: &AppState, track: &Track) -> bool {
    super::current_uid(state).ok() == Some(track.tg_user_id)
}

async fn probe_embedded_artwork(
    state: &AppState,
    app: &AppHandle,
    track: &Track,
    active: Arc<AtomicBool>,
) -> AppResult<Option<Vec<u8>>> {
    let stream = state
        .streaming
        .start(app.clone(), track.clone(), audio_path(state, track)?)
        .await?;
    if stream.total() < 10 {
        return Ok(None);
    }
    let header = stream.read_artwork_range(0, 9, active.clone()).await?;
    let Some(length) = leading_id3_length(&header, stream.total())? else {
        return Ok(None);
    };
    let bytes = stream.read_artwork_range(0, length - 1, active).await?;
    tokio::task::spawn_blocking(move || extract_id3_cover(bytes))
        .await
        .map_err(|error| AppError::msg(error.to_string()))?
}

fn leading_id3_length(header: &[u8], total: u64) -> AppResult<Option<u64>> {
    if header.len() < 10 || &header[..3] != b"ID3" {
        return Ok(None);
    }
    if !(2..=4).contains(&header[3])
        || header[4] == 255
        || header[6..10].iter().any(|byte| byte & 0x80 != 0)
    {
        return Err(AppError::msg("invalid ID3 header"));
    }
    let body = header[6..10]
        .iter()
        .fold(0u64, |size, byte| (size << 7) | u64::from(*byte));
    let footer = if header[3] == 4 && header[5] & 0x10 != 0 {
        10
    } else {
        0
    };
    let length = 10 + body + footer;
    if length > total || length > MAX_ID3_BYTES {
        return Err(AppError::msg("ID3 tag exceeds artwork probe bounds"));
    }
    Ok(Some(length))
}

fn extract_id3_cover(mut bytes: Vec<u8>) -> AppResult<Option<Vec<u8>>> {
    use lofty::{
        config::{ParseOptions, ParsingMode},
        file::{FileType, TaggedFileExt},
        probe::Probe,
    };
    validate_id3_frame_bounds(&bytes)?;
    // This reader receives only verified tag bytes. Local padding lets Lofty's
    // MPEG reader check trailing tag slots without needing any audio packets.
    bytes.extend_from_slice(&[0; 128]);
    let tagged = Probe::with_file_type(std::io::Cursor::new(bytes), FileType::Mpeg)
        .options(
            ParseOptions::new()
                .read_properties(false)
                .parsing_mode(ParsingMode::Strict),
        )
        .read()
        .map_err(|error| AppError::msg(error.to_string()))?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    Ok(tag.and_then(preferred_cover))
}

/// Lofty accepts short picture payloads even in strict mode. Check framing
/// against the verified tag boundary before handing it the tag-only reader.
fn validate_id3_frame_bounds(bytes: &[u8]) -> AppResult<()> {
    use lofty::id3::v2::util::synchsafe::UnsynchronizedStream;
    use std::io::Read;

    let invalid = || AppError::msg("ID3 frame exceeds verified tag bounds");
    let header = bytes.get(..10).ok_or_else(invalid)?;
    let version = header[3];
    let size = header[6..10]
        .iter()
        .fold(0usize, |size, byte| (size << 7) | usize::from(*byte));
    let stored_body = bytes.get(10..10 + size).ok_or_else(invalid)?;
    let mut decoded = Vec::new();
    let body = if version < 4 && header[5] & 0x80 != 0 {
        UnsynchronizedStream::new(stored_body).read_to_end(&mut decoded)?;
        decoded.as_slice()
    } else {
        stored_body
    };
    let mut offset = 0usize;
    if version >= 3 && header[5] & 0x40 != 0 {
        let length: [u8; 4] = body
            .get(..4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        offset = if version == 4 {
            if length.iter().any(|byte| byte & 128 != 0) {
                return Err(invalid());
            }
            length
                .iter()
                .fold(0usize, |size, byte| (size << 7) | usize::from(*byte))
        } else {
            (u32::from_be_bytes(length) as usize)
                .checked_add(4)
                .ok_or_else(invalid)?
        };
        if offset < 6 || offset > body.len() {
            return Err(invalid());
        }
    }
    while offset < body.len() {
        if body[offset] == 0 {
            return if body[offset..].iter().all(|byte| *byte == 0) {
                Ok(())
            } else {
                Err(invalid())
            };
        }
        let header_size = if version == 2 { 6 } else { 10 };
        let frame = body.get(offset..offset + header_size).ok_or_else(invalid)?;
        let size = if version == 2 {
            frame[3..6]
                .iter()
                .fold(0usize, |size, byte| (size << 8) | usize::from(*byte))
        } else if version == 4 {
            if frame[4..8].iter().any(|byte| byte & 128 != 0) {
                return Err(invalid());
            }
            frame[4..8]
                .iter()
                .fold(0usize, |size, byte| (size << 7) | usize::from(*byte))
        } else {
            u32::from_be_bytes(frame[4..8].try_into().map_err(|_| invalid())?) as usize
        };
        if size == 0 || size > body.len() - offset - header_size {
            return Err(invalid());
        }
        offset += header_size + size;
    }
    Ok(())
}

fn preferred_cover(tag: &lofty::tag::Tag) -> Option<Vec<u8>> {
    use lofty::picture::PictureType;
    let picture = tag
        .pictures()
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| tag.pictures().first())?;
    Some(picture.data().to_vec())
}

async fn write_embedded_cover(state: &AppState, dest: &Path, bytes: &[u8]) -> AppResult<()> {
    // Separate from the remote-thumbnail lock: a slow Telegram thumbnail must
    // not delay publishing an embedded image that is already available.
    let lock = state
        .lock_for(&format!("artwork-write:{}", dest.display()))
        .await;
    let _guard = lock.lock().await;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if dest.exists() {
        return Ok(());
    }
    let temporary = dest.with_extension("jpg.part");
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(temporary, dest).await?;
    Ok(())
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
    preferred_cover(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synchsafe(size: usize) -> [u8; 4] {
        [
            ((size >> 21) & 127) as u8,
            ((size >> 14) & 127) as u8,
            ((size >> 7) & 127) as u8,
            (size & 127) as u8,
        ]
    }

    fn tag(version: u8, pictures: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (kind, image) in pictures {
            let mut content = if version == 2 {
                b"\0PNG".to_vec()
            } else {
                b"\0image/png\0".to_vec()
            };
            content.extend_from_slice(&[*kind, 0]);
            content.extend_from_slice(image);
            if version == 2 {
                body.extend_from_slice(b"PIC");
                body.extend_from_slice(&(content.len() as u32).to_be_bytes()[1..]);
            } else {
                body.extend_from_slice(b"APIC");
                body.extend_from_slice(&if version == 4 {
                    synchsafe(content.len())
                } else {
                    (content.len() as u32).to_be_bytes()
                });
                body.extend_from_slice(&[0, 0]);
            }
            body.extend(content);
        }
        let mut bytes = vec![b'I', b'D', b'3', version, 0, 0];
        bytes.extend_from_slice(&synchsafe(body.len()));
        bytes.extend(body);
        bytes
    }

    #[test]
    fn extracts_front_cover_from_tag_only_across_id3_versions() {
        let front = b"\x89PNG\r\n\x1a\nfront".to_vec();
        let back = b"\x89PNG\r\n\x1a\nback".to_vec();
        for version in 2..=4 {
            let bytes = tag(version, &[(4, back.clone()), (3, front.clone())]);
            assert_eq!(
                leading_id3_length(&bytes[..10], bytes.len() as u64).unwrap(),
                Some(bytes.len() as u64)
            );
            assert_eq!(extract_id3_cover(bytes).unwrap(), Some(front.clone()));
        }
    }

    #[test]
    fn extracts_art_larger_than_one_telegram_chunk_without_audio_packets() {
        let mut image = b"\x89PNG\r\n\x1a\n".to_vec();
        image.resize(3 * crate::streaming::CHUNK_SIZE as usize, 42);
        let bytes = tag(4, &[(3, image.clone())]);
        assert_eq!(
            leading_id3_length(&bytes[..10], 20_000_000).unwrap(),
            Some(bytes.len() as u64)
        );
        assert_eq!(extract_id3_cover(bytes).unwrap(), Some(image));
    }

    #[test]
    fn accepts_missing_art_but_rejects_truncated_malformed_and_oversized_tags() {
        assert_eq!(leading_id3_length(b"fLaC000000", 100).unwrap(), None);
        assert_eq!(extract_id3_cover(tag(3, &[])).unwrap(), None);
        let mut header = b"ID3\x04\0\0\0\0\0\x20".to_vec();
        assert!(leading_id3_length(&header, 20).is_err());
        header[6..10].copy_from_slice(&synchsafe(MAX_ID3_BYTES as usize));
        assert!(leading_id3_length(&header, 20_000_000).is_err());
        header[6] = 128;
        assert!(leading_id3_length(&header, 20_000_000).is_err());
        header[3] = 5;
        assert!(leading_id3_length(&header, 20_000_000).is_err());
    }

    #[test]
    fn includes_v24_footer_in_verified_range() {
        assert_eq!(
            leading_id3_length(b"ID3\x04\0\x10\0\0\0\x20", 100).unwrap(),
            Some(52)
        );
    }

    #[test]
    fn handles_v23_unsynchronization_and_rejects_truncated_picture_frames() {
        let image = b"\x89PNG\r\n\x1a\n\xff\xe0image".to_vec();
        let bytes = tag(3, &[(3, image.clone())]);
        let mut body = Vec::new();
        for byte in &bytes[10..] {
            body.push(*byte);
            if *byte == 255 {
                body.push(0);
            }
        }
        let mut unsynchronized = bytes[..10].to_vec();
        unsynchronized[5] = 0x80;
        unsynchronized[6..10].copy_from_slice(&synchsafe(body.len()));
        unsynchronized.extend(body);
        assert_eq!(extract_id3_cover(unsynchronized).unwrap(), Some(image));

        let mut truncated = bytes;
        truncated.truncate(truncated.len() - 5);
        let body_size = truncated.len() - 10;
        truncated[6..10].copy_from_slice(&synchsafe(body_size));
        assert!(extract_id3_cover(truncated).is_err());
    }
}
