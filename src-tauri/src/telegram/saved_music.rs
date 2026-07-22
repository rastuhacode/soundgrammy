//! Saved-music synchronization: paginated `users.GetSavedMusic` with XOR-hash
//! change detection, mirroring the web app's sync logic.

#![allow(deprecated)]

use ferogram::tl;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db::UpsertTrack;
use crate::error::AppResult;
use crate::state::AppState;
use crate::telegram::auth;
use crate::telegram::document::{compute_saved_music_hash, parse_document};

const PAGE_LIMIT: i32 = 100;

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub changed: bool,
    pub total: i64,
    #[serde(rename = "lastSyncAt")]
    pub last_sync_at: Option<String>,
}

#[derive(Clone, Serialize)]
struct SyncProgress {
    done: usize,
    total: i32,
}

/// Fetches the account's saved music and reconciles it into the local library.
pub async fn sync(state: &AppState, app: &AppHandle) -> AppResult<SyncResult> {
    let user = auth::fetch_self(&state.client).await?;
    let uid = user.id;
    state.db.save_profile(
        uid,
        &user.first_name,
        user.last_name.as_deref(),
        user.username.as_deref(),
        user.phone.as_deref(),
    )?;

    let prev_hash = state
        .db
        .saved_music_hash(uid)?
        .and_then(|h| h.parse::<i64>().ok())
        .unwrap_or(0);

    let _ = app.emit("sync:start", ());

    let mut collected: Vec<tl::types::Document> = Vec::new();
    let mut offset = 0i32;

    loop {
        let hash = if offset == 0 { prev_hash } else { 0 };
        let result = state
            .client
            .invoke(&tl::functions::users::GetSavedMusic {
                id: tl::enums::InputUser::UserSelf,
                offset,
                limit: PAGE_LIMIT,
                hash,
            })
            .await?;

        match result {
            tl::enums::users::SavedMusic::NotModified(n) => {
                state.db.mark_synced(uid)?;
                let _ = app.emit("sync:done", ());
                return Ok(SyncResult {
                    changed: false,
                    total: n.count as i64,
                    last_sync_at: state.db.last_sync_at(uid)?,
                });
            }
            tl::enums::users::SavedMusic::SavedMusic(m) => {
                let total_count = m.count;
                let before = collected.len();
                for document in m.documents {
                    if let tl::enums::Document::Document(d) = document {
                        collected.push(d);
                    }
                }
                let got = collected.len() - before;
                let _ = app.emit(
                    "sync:progress",
                    SyncProgress {
                        done: collected.len(),
                        total: total_count,
                    },
                );
                if got < PAGE_LIMIT as usize || collected.len() as i32 >= total_count {
                    break;
                }
                offset += got as i32;
            }
        }
    }

    let mut ids: Vec<i64> = Vec::with_capacity(collected.len());
    let mut keep_unique: Vec<String> = Vec::with_capacity(collected.len());
    for (position, document) in collected.iter().enumerate() {
        let parsed = parse_document(document);
        ids.push(document.id);
        keep_unique.push(parsed.file_unique_id.clone());
        state.db.upsert_track(&UpsertTrack {
            tg_user_id: uid,
            file_id: parsed.file_id,
            file_unique_id: parsed.file_unique_id,
            title: parsed.title,
            performer: parsed.performer,
            duration: parsed.duration,
            mime_type: Some(parsed.mime_type),
            file_size: Some(parsed.size),
            track_position: position as i64,
            mtproto_document: parsed.stored_json,
        })?;
    }

    state.db.delete_tracks_not_in(uid, &keep_unique)?;
    let new_hash = compute_saved_music_hash(&ids);
    state.db.set_saved_music_hash(uid, &new_hash.to_string())?;
    state.db.mark_synced(uid)?;

    let _ = app.emit("sync:done", ());

    Ok(SyncResult {
        changed: true,
        total: ids.len() as i64,
        last_sync_at: state.db.last_sync_at(uid)?,
    })
}
