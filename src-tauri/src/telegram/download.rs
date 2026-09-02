//! Downloading documents and thumbnails, with file-reference refresh.
//!
//! Chunk fetches go through [`Client::invoke_on_dc`] with `upload.getFile`

use std::path::Path;
use std::sync::atomic::AtomicBool;

use ferogram::cdn_download::{CdnChunkResult, CdnDownloader, CDN_CHUNK_SIZE};
use ferogram::tl;
use ferogram::{Client, InvocationError};

use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::{parse_document, StoredDocument};
use crate::telegram::media_requests::{MediaPriority, MediaRequestCoordinator};

const CHUNK_SIZE: i32 = 128 * 1024;

#[derive(Clone, Copy)]
struct MediaRequestContext<'a> {
    coordinator: &'a MediaRequestCoordinator,
    priority: MediaPriority,
    active: Option<&'a AtomicBool>,
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

/// Downloads one aligned 128 KiB streaming part via pooled `upload.getFile`.
pub(crate) async fn download_chunk(
    client: &Client,
    coordinator: &MediaRequestCoordinator,
    document: &StoredDocument,
    chunk_index: usize,
    active: Option<&AtomicBool>,
) -> Result<Vec<u8>, InvocationError> {
    let chunk_index = i64::try_from(chunk_index).map_err(|_| {
        InvocationError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "audio chunk index is too large",
        ))
    })?;
    let offset = chunk_index * i64::from(CHUNK_SIZE);
    get_file_bytes(
        client,
        MediaRequestContext {
            coordinator,
            priority: if active.is_some() {
                MediaPriority::Playback
            } else {
                MediaPriority::Background
            },
            active,
        },
        document.dc_id,
        document.input_location(),
        offset,
        CHUNK_SIZE,
    )
    .await
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
    let client = state.client().await?;

    match download_location_bytes(
        &client,
        MediaRequestContext {
            coordinator: &state.media_requests,
            priority: MediaPriority::Background,
            active: None,
        },
        doc.dc_id,
        location.clone(),
    )
    .await
    {
        Ok(bytes) => {
            tokio::fs::write(&part, &bytes).await?;
        }
        Err(err) if is_file_reference_error(&err) => {
            let refreshed = refresh_file_reference(state, track).await?;
            let Some(location) = refreshed.thumb_input_location_for(high_quality) else {
                return Ok(false);
            };
            let client = state.client().await?;
            let bytes = download_location_bytes(
                &client,
                MediaRequestContext {
                    coordinator: &state.media_requests,
                    priority: MediaPriority::Background,
                    active: None,
                },
                refreshed.dc_id,
                location,
            )
            .await?;
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
    let client = state.client().await?;
    let result = client
        .invoke(&tl::functions::users::GetSavedMusicById {
            id: tl::enums::InputUser::UserSelf,
            documents: vec![doc.input_document()],
        })
        .await?;

    let documents = match result {
        tl::enums::users::SavedMusic::SavedMusic(m) => m.documents,
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

/// Downloads a location (e.g. profile photos) to `dest`.
///
/// `dc_id` must be the file's real DC (e.g. `UserProfilePhoto.dc_id`). Do not
/// pass `0`: [`Client::invoke_on_dc`] does not map 0 to the home DC.
pub async fn download_location(
    state: &AppState,
    dc_id: i32,
    location: tl::enums::InputFileLocation,
    dest: &Path,
) -> AppResult<()> {
    let client = state.client().await?;
    let bytes = download_location_bytes(
        &client,
        MediaRequestContext {
            coordinator: &state.media_requests,
            priority: MediaPriority::Background,
            active: None,
        },
        dc_id,
        location,
    )
    .await?;
    let part = with_part_extension(dest);
    tokio::fs::write(&part, &bytes).await?;
    tokio::fs::rename(&part, dest).await?;
    Ok(())
}

async fn download_location_bytes(
    client: &Client,
    context: MediaRequestContext<'_>,
    dc_id: i32,
    location: tl::enums::InputFileLocation,
) -> Result<Vec<u8>, InvocationError> {
    let mut offset = 0i64;
    let mut bytes = Vec::new();
    loop {
        let chunk =
            get_file_bytes(client, context, dc_id, location.clone(), offset, CHUNK_SIZE).await?;
        let n = chunk.len() as i32;
        bytes.extend_from_slice(&chunk);
        if n < CHUNK_SIZE {
            break;
        }
        offset += i64::from(CHUNK_SIZE);
    }
    Ok(bytes)
}

/// One `upload.getFile` on the pooled DC connection (with FILE_MIGRATE + CDN follow).
async fn get_file_bytes(
    client: &Client,
    context: MediaRequestContext<'_>,
    mut dc_id: i32,
    location: tl::enums::InputFileLocation,
    offset: i64,
    limit: i32,
) -> Result<Vec<u8>, InvocationError> {
    loop {
        let req = tl::functions::upload::GetFile {
            precise: true,
            // Advertise CDN support so Telegram may redirect; we follow below.
            cdn_supported: true,
            location: location.clone(),
            offset,
            limit,
        };
        let permit = context
            .coordinator
            .acquire(dc_id, context.priority, context.active)
            .await?;
        let result = client.invoke_on_dc(dc_id, &req).await;
        if let Err(error) = &result {
            if context.coordinator.observe_flood_wait(dc_id, error).await {
                tracing::warn!(
                    "Telegram media DC{dc_id} requested a flood wait ({error}); delaying media requests"
                );
                drop(permit);
                continue;
            }
        }
        drop(permit);
        match result {
            Ok(tl::enums::upload::File::File(f)) => return Ok(f.bytes),
            Ok(tl::enums::upload::File::CdnRedirect(redir)) => {
                return get_cdn_file_bytes(client, context, dc_id, redir, offset, limit).await;
            }
            Err(InvocationError::Rpc(rpc))
                if rpc.name.contains("FILE_MIGRATE")
                    || (rpc.code == 303 && rpc.name.contains("MIGRATE")) =>
            {
                let new_dc = rpc.value.unwrap_or(0) as i32;
                if new_dc == 0 || new_dc == dc_id {
                    return Err(InvocationError::Rpc(rpc));
                }
                dc_id = new_dc;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Fetch one chunk after `upload.fileCdnRedirect` (AES-CTR CDN DC path).
async fn get_cdn_file_bytes(
    client: &Client,
    context: MediaRequestContext<'_>,
    media_dc_id: i32,
    redir: tl::types::upload::FileCdnRedirect,
    offset: i64,
    limit: i32,
) -> Result<Vec<u8>, InvocationError> {
    let addr = client.media_dc_addr(redir.dc_id).await.ok_or_else(|| {
        InvocationError::Deserialize(format!(
            "no address for CDN DC{}; cannot follow fileCdnRedirect",
            redir.dc_id
        ))
    })?;

    let key: [u8; 32] =
        redir.encryption_key.as_slice().try_into().map_err(|_| {
            InvocationError::Deserialize("CDN encryption_key must be 32 bytes".into())
        })?;
    let iv: [u8; 16] =
        redir.encryption_iv.as_slice().try_into().map_err(|_| {
            InvocationError::Deserialize("CDN encryption_iv must be 16 bytes".into())
        })?;

    // CDN part size is fixed at 128 KiB; clamp the request to that.
    let cdn_limit = if limit <= 0 || limit > CDN_CHUNK_SIZE {
        CDN_CHUNK_SIZE
    } else {
        limit
    };

    let mut downloader = CdnDownloader::connect(
        &addr,
        redir.dc_id as i16,
        redir.file_token.clone(),
        key,
        iv,
        None,
    )
    .await?;

    loop {
        let permit = context
            .coordinator
            .acquire(redir.dc_id, context.priority, context.active)
            .await?;
        let result = downloader.download_chunk_raw(offset, cdn_limit).await;
        if let Err(error) = &result {
            if context
                .coordinator
                .observe_flood_wait(redir.dc_id, error)
                .await
            {
                tracing::warn!(
                    "Telegram CDN DC{} requested a flood wait ({error}); delaying media requests",
                    redir.dc_id,
                );
                drop(permit);
                continue;
            }
        }
        drop(permit);
        let chunk = match result {
            Ok(chunk) => chunk,
            Err(error) => return Err(error),
        };
        match chunk {
            CdnChunkResult::Data(bytes) => return Ok(bytes),
            CdnChunkResult::ReuploadNeeded(request_token) => {
                reupload_cdn_file(
                    client,
                    context,
                    media_dc_id,
                    redir.file_token.clone(),
                    request_token,
                )
                .await?;
                // Retry the same offset after reupload.
            }
        }
    }
}

async fn reupload_cdn_file(
    client: &Client,
    context: MediaRequestContext<'_>,
    media_dc_id: i32,
    file_token: Vec<u8>,
    request_token: Vec<u8>,
) -> Result<(), InvocationError> {
    loop {
        let permit = context
            .coordinator
            .acquire(media_dc_id, context.priority, context.active)
            .await?;
        let result = client
            .invoke_on_dc(
                media_dc_id,
                &tl::functions::upload::ReuploadCdnFile {
                    file_token: file_token.clone(),
                    request_token: request_token.clone(),
                },
            )
            .await;
        if let Err(error) = &result {
            if context
                .coordinator
                .observe_flood_wait(media_dc_id, error)
                .await
            {
                tracing::warn!(
                    "Telegram media DC{media_dc_id} requested a flood wait during CDN reupload ({error})"
                );
                drop(permit);
                continue;
            }
        }
        drop(permit);
        match result {
            Ok(_) => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn with_part_extension(path: &Path) -> std::path::PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(".part");
    std::path::PathBuf::from(os)
}
