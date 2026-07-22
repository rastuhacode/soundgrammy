//! Downloading documents and thumbnails, with file-reference refresh.
//!
//! Chunk fetches go through [`Client::invoke_on_dc`] with `upload.getFile`, the
//! same pattern grammers used. ferogram's `iter_download` opens a dedicated
//! worker TCP connection per iterator; creating one iterator per 128 KiB chunk
//! (as streaming does) caused multi-second reconnect overhead and hangs under
//! concurrent range requests.

use std::path::Path;

use ferogram::tl;
use ferogram::{Client, InvocationError};

use crate::db::Track;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::document::{parse_document, StoredDocument};

const CHUNK_SIZE: i32 = 128 * 1024;

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
    document: &StoredDocument,
    chunk_index: usize,
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

    match download_location_bytes(&state.client, doc.dc_id, location.clone()).await {
        Ok(bytes) => {
            tokio::fs::write(&part, &bytes).await?;
        }
        Err(err) if is_file_reference_error(&err) => {
            let refreshed = refresh_file_reference(state, track).await?;
            let Some(location) = refreshed.thumb_input_location_for(high_quality) else {
                return Ok(false);
            };
            let bytes = download_location_bytes(&state.client, refreshed.dc_id, location).await?;
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
pub async fn download_location(
    client: &Client,
    location: tl::enums::InputFileLocation,
    dest: &Path,
) -> AppResult<()> {
    // Profile photos live on the home DC; dc_id 0 lets ferogram resolve home.
    let bytes = download_location_bytes(client, 0, location).await?;
    let part = with_part_extension(dest);
    tokio::fs::write(&part, &bytes).await?;
    tokio::fs::rename(&part, dest).await?;
    Ok(())
}

async fn download_location_bytes(
    client: &Client,
    dc_id: i32,
    location: tl::enums::InputFileLocation,
) -> Result<Vec<u8>, InvocationError> {
    let mut offset = 0i64;
    let mut bytes = Vec::new();
    loop {
        let chunk = get_file_bytes(client, dc_id, location.clone(), offset, CHUNK_SIZE).await?;
        let n = chunk.len() as i32;
        bytes.extend_from_slice(&chunk);
        if n < CHUNK_SIZE {
            break;
        }
        offset += i64::from(CHUNK_SIZE);
    }
    Ok(bytes)
}

/// One `upload.getFile` on the pooled DC connection (with FILE_MIGRATE follow).
async fn get_file_bytes(
    client: &Client,
    mut dc_id: i32,
    location: tl::enums::InputFileLocation,
    offset: i64,
    limit: i32,
) -> Result<Vec<u8>, InvocationError> {
    loop {
        let req = tl::functions::upload::GetFile {
            precise: true,
            cdn_supported: false,
            location: location.clone(),
            offset,
            limit,
        };
        match client.invoke_on_dc(dc_id, &req).await {
            Ok(tl::enums::upload::File::File(f)) => return Ok(f.bytes),
            Ok(tl::enums::upload::File::CdnRedirect(_)) => {
                return Err(InvocationError::Deserialize(
                    "upload.fileCdnRedirect received (cdn_supported=false was ignored)".into(),
                ));
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

fn with_part_extension(path: &Path) -> std::path::PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(".part");
    std::path::PathBuf::from(os)
}
