//! Downloading documents and thumbnails, with file-reference refresh.
//!
//! grammers' `iter_download` transparently handles DC migration and copying the
//! authorization to the target DC, but it does *not* refresh expired file
//! references. We detect `FILE_REFERENCE_EXPIRED` and retry once after fetching
//! a fresh document via `users.GetSavedMusicByID`.

use std::path::Path;

use grammers_client::media::Downloadable;
use grammers_client::Client;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;

use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::{parse_document, StoredDocument};

/// A raw, location-based downloadable for documents and their thumbnails.
struct RawDownloadable {
    location: tl::enums::InputFileLocation,
    size: Option<usize>,
}

impl Downloadable for RawDownloadable {
    fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        Some(self.location.clone())
    }

    fn size(&self) -> Option<usize> {
        self.size
    }
}

pub(crate) fn is_file_reference_error(err: &InvocationError) -> bool {
    matches!(err, InvocationError::Rpc(e) if e.name.contains("FILE_REFERENCE"))
}

pub fn stored_document(track: &Track) -> AppResult<StoredDocument> {
    let json = track
        .mtproto_document
        .as_deref()
        .ok_or_else(|| AppError::msg("track has no Telegram document"))?;
    Ok(serde_json::from_str(json)?)
}

/// Downloads one aligned 128 KiB streaming part.
pub(crate) async fn download_chunk(
    client: &Client,
    document: &StoredDocument,
    chunk_index: usize,
) -> Result<Vec<u8>, InvocationError> {
    const CHUNK_SIZE: i32 = 128 * 1024;
    let chunk_index = i32::try_from(chunk_index).map_err(|_| {
        InvocationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "audio chunk index is too large",
        ))
    })?;
    let downloadable = RawDownloadable {
        location: document.input_location(),
        size: Some(document.size_bytes() as usize),
    };
    let mut download = client
        .iter_download(&downloadable)
        .chunk_size(CHUNK_SIZE)
        .skip_chunks(chunk_index);
    Ok(download.next().await?.unwrap_or_default())
}

/// Downloads the remote thumbnail (if the document has one) to `dest`.
/// Returns `false` when the document has no remote thumbnail.
pub async fn download_thumbnail(
    state: &AppState,
    track: &Track,
    dest: &Path,
    high_quality: bool,
) -> AppResult<bool> {
    let mut doc = stored_document(track)?;
    if high_quality && doc.thumbnails.is_empty() {
        // Older rows only persisted one medium thumbnail. Refresh once so a
        // fullscreen request can discover the complete Telegram size list.
        if let Ok(refreshed) = refresh_file_reference(state, track).await {
            doc = refreshed;
        }
    }
    let Some(location) = doc.thumb_input_location_for(high_quality) else {
        return Ok(false);
    };
    let part = with_part_extension(dest);

    match download_bytes(&state.client, location.clone()).await {
        Ok(bytes) => {
            tokio::fs::write(&part, &bytes).await?;
        }
        Err(err) if is_file_reference_error(&err) => {
            let refreshed = refresh_file_reference(state, track).await?;
            let Some(location) = refreshed.thumb_input_location_for(high_quality) else {
                return Ok(false);
            };
            let bytes = download_bytes(&state.client, location).await?;
            tokio::fs::write(&part, &bytes).await?;
        }
        Err(err) => return Err(err.into()),
    }

    tokio::fs::rename(&part, dest).await?;
    Ok(true)
}

/// Fetches a fresh document (renewed file reference) via GetSavedMusicByID and
/// persists it back to the track row.
pub(crate) async fn refresh_file_reference(
    state: &AppState,
    track: &Track,
) -> AppResult<StoredDocument> {
    let doc = stored_document(track)?;
    let result = state
        .client
        .invoke(&tl::functions::users::GetSavedMusicById {
            id: tl::enums::InputUser::UserSelf,
            documents: vec![doc.input_document()],
        })
        .await?;

    let documents = match result {
        tl::enums::users::SavedMusic::Music(m) => m.documents,
        tl::enums::users::SavedMusic::NotModified(_) => Vec::new(),
    };

    for d in documents {
        if let tl::enums::Document::Document(document) = d {
            if document.id == doc.id {
                let parsed = parse_document(&document);
                state
                    .db
                    .update_track_document(track.id, track.tg_user_id, &parsed.stored_json)?;
                return Ok(serde_json::from_str(&parsed.stored_json)?);
            }
        }
    }

    Err(AppError::msg(
        "could not refresh the track's file reference",
    ))
}

/// Downloads a location with no file reference (e.g. profile photos) to `dest`.
pub async fn download_location(
    client: &Client,
    location: tl::enums::InputFileLocation,
    dest: &Path,
) -> AppResult<()> {
    let bytes = download_bytes(client, location).await?;
    let part = with_part_extension(dest);
    tokio::fs::write(&part, &bytes).await?;
    tokio::fs::rename(&part, dest).await?;
    Ok(())
}

async fn download_bytes(
    client: &Client,
    location: tl::enums::InputFileLocation,
) -> Result<Vec<u8>, InvocationError> {
    let downloadable = RawDownloadable {
        location,
        size: None,
    };
    let mut iter = client.iter_download(&downloadable);
    let mut bytes = Vec::new();
    while let Some(chunk) = iter.next().await? {
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn with_part_extension(path: &Path) -> std::path::PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(".part");
    std::path::PathBuf::from(os)
}
