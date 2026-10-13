//! Progressive saved-music indexing, gated by the lightweight account manifest.

#![allow(deprecated)]

use std::collections::HashSet;

use async_trait::async_trait;
use ferogram::tl;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::db::{Db, LibraryBatch, UpsertTrack};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::telegram::{
    auth,
    document::{compute_saved_music_hash, parse_document},
};

const PAGE_LIMIT: i32 = 100;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncMode {
    #[default]
    Soft,
    Full,
}

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
    total: i64,
}

#[derive(Clone, Serialize)]
struct SyncError {
    message: String,
}

struct MusicPage {
    total: i64,
    tracks: Vec<UpsertTrack>,
}

/// A narrow request boundary allows the real reconciliation workflow to be
/// tested with failures, changing offsets, and large catalogues without Telegram.
#[async_trait]
trait MusicSource: Sync {
    async fn manifest(&self, hash: i64) -> AppResult<Option<Vec<i64>>>;
    async fn page(&self, offset: i32) -> AppResult<MusicPage>;
}

struct TelegramSource<'a> {
    state: &'a AppState,
    uid: i64,
}

impl TelegramSource<'_> {
    fn check_account(&self) -> AppResult<()> {
        if self
            .state
            .db
            .load_profile()?
            .is_none_or(|p| p.tg_user_id != self.uid)
        {
            return Err(AppError::NotAuthorized);
        }
        let wait = self
            .state
            .db
            .saved_music_wait_seconds(self.uid, now_seconds()?)?;
        if wait > 0 {
            return Err(flood_wait_message(wait));
        }
        Ok(())
    }

    fn rpc_result<T>(&self, result: Result<T, ferogram::InvocationError>) -> AppResult<T> {
        self.check_account()?;
        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                if let Some(seconds) = flood_wait_seconds(&error) {
                    // Long waits escape ferogram's automatic sleep threshold.
                    // Persist the deadline so retries/restarts respect it too.
                    // Whole-second timestamps need a little rounding grace.
                    self.state.db.defer_saved_music_sync(
                        self.uid,
                        now_seconds()?.saturating_add(seconds).saturating_add(1),
                    )?;
                    return Err(flood_wait_message(seconds));
                }
                Err(error.into())
            }
        }
    }
}

fn now_seconds() -> AppResult<i64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AppError::msg("System time is before the Unix epoch"))?
        .as_secs() as i64)
}

fn flood_wait_message(seconds: i64) -> AppError {
    AppError::msg(format!(
        "Telegram requested a flood wait. Retry sync in {seconds}s."
    ))
}

fn flood_wait_seconds(error: &ferogram::InvocationError) -> Option<i64> {
    let ferogram::InvocationError::Rpc(rpc) = error else {
        return None;
    };
    if rpc.code != 420 || !matches!(rpc.name.as_str(), "FLOOD_WAIT" | "FLOOD_PREMIUM_WAIT") {
        return None;
    }
    rpc.value.filter(|seconds| *seconds > 0).map(i64::from)
}

#[async_trait]
impl MusicSource for TelegramSource<'_> {
    async fn manifest(&self, hash: i64) -> AppResult<Option<Vec<i64>>> {
        self.check_account()?;
        // ferogram invokes respect Telegram flood waits and retry transient RPC failures.
        let result = self
            .state
            .client()
            .await?
            .invoke(&tl::functions::account::GetSavedMusicIds { hash })
            .await;
        let result = self.rpc_result(result)?;
        Ok(match result {
            tl::enums::account::SavedMusicIds::NotModified => None,
            tl::enums::account::SavedMusicIds::SavedMusicIds(m) => Some(m.ids),
        })
    }

    async fn page(&self, offset: i32) -> AppResult<MusicPage> {
        self.check_account()?;
        // The manifest, not a first-page hash, establishes whole-library freshness.
        let result = self
            .state
            .client()
            .await?
            .invoke(&tl::functions::users::GetSavedMusic {
                id: tl::enums::InputUser::UserSelf,
                offset,
                limit: PAGE_LIMIT,
                hash: 0,
            })
            .await;
        let result = self.rpc_result(result)?;
        let tl::enums::users::SavedMusic::SavedMusic(m) = result else {
            return Err(AppError::msg("Unexpected saved-music page response"));
        };
        let mut tracks = Vec::with_capacity(m.documents.len());
        for (index, document) in m.documents.into_iter().enumerate() {
            let tl::enums::Document::Document(document) = document else {
                return Err(AppError::msg("Incomplete saved-music document; retry"));
            };
            let parsed = parse_document(&document);
            tracks.push(UpsertTrack {
                tg_user_id: self.uid,
                file_id: parsed.file_id,
                file_unique_id: parsed.file_unique_id,
                title: parsed.title,
                title_source: parsed.title_source,
                performer: parsed.performer,
                duration: parsed.duration,
                mime_type: Some(parsed.mime_type),
                file_size: Some(parsed.size),
                track_position: i64::from(offset) + index as i64,
                mtproto_document: parsed.stored_json,
            });
        }
        Ok(MusicPage {
            total: i64::from(m.count),
            tracks,
        })
    }
}

pub async fn sync(state: &AppState, app: &AppHandle, mode: SyncMode) -> AppResult<SyncResult> {
    let _guard = state.sync_lock.lock().await;
    let _ = app.emit("sync:start", ());
    let result = async {
        let uid = state
            .db
            .load_profile()?
            .ok_or(AppError::NotAuthorized)?
            .tg_user_id;
        let source = TelegramSource { state, uid };
        if mode == SyncMode::Full {
            let batch = state.db.request_full_reindex(uid)?;
            let _ = app.emit("library:batch", batch);
        }
        source.check_account()?;
        let user = auth::fetch_self(&state.client().await?).await?;
        source.check_account()?;
        if user.id != uid {
            return Err(AppError::NotAuthorized);
        }
        state.db.refresh_active_profile(
            uid,
            &user.first_name,
            user.last_name.as_deref(),
            user.username.as_deref(),
            user.phone.as_deref(),
        )?;
        let publish = |batch: LibraryBatch, done| {
            let _ = app.emit(
                "sync:progress",
                SyncProgress {
                    done,
                    total: batch.status.total,
                },
            );
            let _ = app.emit("library:batch", batch);
        };
        if state.db.library_status(uid)?.blocking {
            index(&state.db, uid, &source, publish).await
        } else {
            soft_index(&state.db, uid, &source, publish).await
        }
    }
    .await;
    match &result {
        Ok(_) => {
            let _ = app.emit("sync:done", ());
        }
        Err(error) => {
            let _ = app.emit(
                "sync:error",
                SyncError {
                    message: error.to_string(),
                },
            );
        }
    }
    result
}

fn validate_manifest(ids: &[i64]) -> AppResult<HashSet<String>> {
    let unique: HashSet<_> = ids.iter().map(ToString::to_string).collect();
    if unique.len() != ids.len() || ids.len() > i32::MAX as usize {
        return Err(AppError::msg("Invalid saved-music manifest"));
    }
    Ok(unique)
}

async fn index(
    db: &Db,
    uid: i64,
    source: &impl MusicSource,
    mut publish: impl FnMut(LibraryBatch, usize),
) -> AppResult<SyncResult> {
    let mut publish = |mut batch: LibraryBatch, done| {
        // Full indexing exposes progress, never a partial catalogue.
        batch.tracks.clear();
        publish(batch, done);
    };
    let checkpoint = db.saved_music_checkpoint(uid)?;
    let mut response = source
        .manifest(checkpoint.as_ref().map_or(0, |c| c.hash))
        .await?;
    if response.is_none() && checkpoint.is_none() {
        // An empty list also hashes to zero. Ask with a different hash rather
        // than treating an uncached NotModified response as a complete index.
        response = source.manifest(-1).await?;
    }
    let ids = response
        .or_else(|| checkpoint.as_ref().map(|c| c.manifest.clone()))
        .ok_or_else(|| AppError::msg("Saved-music manifest not cached; retry"))?;
    let expected = validate_manifest(&ids)?;
    let hash = compute_saved_music_hash(&ids);
    let same = checkpoint.as_ref().is_some_and(|c| c.manifest == ids);
    if same && checkpoint.as_ref().is_some_and(|c| c.complete) {
        // This branch is only reachable with a complete metadata/order catalogue.
        db.mark_synced(uid)?;
        return Ok(SyncResult {
            changed: false,
            total: ids.len() as i64,
            last_sync_at: db.library_status(uid)?.last_sync_at,
        });
    }

    let resume = same && checkpoint.as_ref().is_some_and(|c| !c.complete);
    let mut offset = if resume {
        checkpoint.as_ref().unwrap().next_offset
    } else {
        0
    };
    if !resume {
        publish(db.begin_saved_music_scan(uid, &ids, hash)?, 0);
    }

    // Offsets can shift while offline. Re-read an overlapping boundary before
    // trusting a durable cursor; the final manifest check validates the full run.
    if offset > 0 {
        let start = (offset - PAGE_LIMIT).max(0);
        let boundary = source.page(start).await?;
        let prior = db.saved_music_scan_ids(uid, start, offset - start)?;
        let current = boundary
            .tracks
            .iter()
            .take(prior.len())
            .map(|t| t.file_unique_id.parse::<i64>().ok())
            .collect::<Option<Vec<_>>>();
        if boundary.total != ids.len() as i64 || current.as_ref() != Some(&prior) {
            publish(db.begin_saved_music_scan(uid, &ids, hash)?, 0);
            offset = 0;
        }
    }
    let mut seen: HashSet<String> = db
        .saved_music_scan_ids(uid, 0, offset)?
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    while (offset as usize) < ids.len() {
        let page = source.page(offset).await?;
        let invalid = page.total != ids.len() as i64
            || page.tracks.is_empty()
            || page.tracks.len() > PAGE_LIMIT as usize
            || offset as usize + page.tracks.len() > ids.len()
            || page.tracks.iter().any(|t| {
                !expected.contains(&t.file_unique_id) || !seen.insert(t.file_unique_id.clone())
            });
        if invalid {
            // Keep metadata, but don't keep a cursor from a shifting/invalid scan.
            publish(db.begin_saved_music_scan(uid, &ids, hash)?, 0);
            return Err(AppError::msg("Saved music changed during indexing; retry"));
        }
        let batch = db.append_saved_music_page(uid, offset, &page.tracks)?;
        offset += page.tracks.len() as i32;
        publish(batch, offset as usize);
        // Yield between pages so sync cannot starve playback or local queries.
        tokio::task::yield_now().await;
    }

    if let Some(latest) = source.manifest(hash).await? {
        if latest != ids {
            validate_manifest(&latest)?;
            publish(
                db.begin_saved_music_scan(uid, &latest, compute_saved_music_hash(&latest))?,
                0,
            );
            return Err(AppError::msg("Saved music changed during indexing; retry"));
        }
    }
    let batch = db.finish_saved_music_scan(uid)?;
    let last_sync_at = batch.status.last_sync_at.clone();
    publish(batch, ids.len());
    Ok(SyncResult {
        changed: true,
        total: ids.len() as i64,
        last_sync_at,
    })
}

/// A soft sync reads the manifest and fetches only pages needed to discover
/// missing documents. Known metadata is never replaced and no full scan is
/// scheduled based on elapsed time. Full reindexing is an explicit user action.
async fn soft_index(
    db: &Db,
    uid: i64,
    source: &impl MusicSource,
    mut publish: impl FnMut(LibraryBatch, usize),
) -> AppResult<SyncResult> {
    let checkpoint = db.saved_music_checkpoint(uid)?;
    let mut response = source
        .manifest(checkpoint.as_ref().map_or(0, |c| c.hash))
        .await?;
    if response.is_none() && checkpoint.is_none() {
        response = source.manifest(-1).await?;
    }
    let ids = response
        .or_else(|| checkpoint.as_ref().map(|c| c.manifest.clone()))
        .ok_or_else(|| AppError::msg("Saved-music manifest not cached; retry"))?;
    let expected = validate_manifest(&ids)?;
    let hash = compute_saved_music_hash(&ids);
    let same = checkpoint.as_ref().is_some_and(|c| c.manifest == ids);
    if same
        && checkpoint.as_ref().is_some_and(|c| c.complete)
        && db.missing_saved_music_ids(uid)?.is_empty()
    {
        db.mark_synced(uid)?;
        return Ok(SyncResult {
            changed: false,
            total: ids.len() as i64,
            last_sync_at: db.last_sync_at(uid)?,
        });
    }
    if !same
        || checkpoint
            .as_ref()
            .is_some_and(|c| c.blocking || c.complete)
    {
        let batch = db.begin_saved_music_scan_mode(uid, &ids, hash, false)?;
        let done = batch.status.indexed as usize;
        publish(batch, done);
    }
    let mut requested = HashSet::new();
    let mut continuations = std::collections::VecDeque::new();
    loop {
        let missing = db.missing_saved_music_ids(uid)?;
        if missing.is_empty() {
            break;
        }
        // Ordinals are cheap hints. If they don't match saved-music order,
        // search the remaining pages rather than treating an unseen ID as gone.
        let offset = missing
            .iter()
            .map(|(_, ordinal)| ordinal / PAGE_LIMIT * PAGE_LIMIT)
            .find(|offset| !requested.contains(offset))
            .or_else(|| {
                while let Some(offset) = continuations.pop_front() {
                    if !requested.contains(&offset) {
                        return Some(offset);
                    }
                }
                None
            })
            .or_else(|| {
                (0..ids.len() as i32)
                    .step_by(PAGE_LIMIT as usize)
                    .find(|offset| !requested.contains(offset))
            })
            .ok_or_else(|| AppError::msg("Could not find all saved-music documents; retry"))?;
        requested.insert(offset);
        let page = source.page(offset).await?;
        let mut unique = HashSet::new();
        if page.total != ids.len() as i64
            || page.tracks.is_empty()
            || page.tracks.len() > PAGE_LIMIT as usize
            || offset as usize + page.tracks.len() > ids.len()
            || page.tracks.iter().any(|t| {
                !expected.contains(&t.file_unique_id) || !unique.insert(t.file_unique_id.clone())
            })
        {
            return Err(AppError::msg("Saved music changed during sync; retry"));
        }
        if page.tracks.len() < PAGE_LIMIT as usize
            && offset as usize + page.tracks.len() < ids.len()
        {
            continuations.push_back(offset + page.tracks.len() as i32);
        }
        let missing: HashSet<_> = missing.into_iter().map(|(id, _)| id).collect();
        let tracks: Vec<_> = page
            .tracks
            .into_iter()
            .filter(|t| missing.contains(&t.file_unique_id))
            .collect();
        if !tracks.is_empty() {
            let batch = db.append_saved_music_missing(uid, &tracks)?;
            let done = batch.status.indexed as usize;
            publish(batch, done);
        }
        tokio::task::yield_now().await;
    }
    if let Some(latest) = source.manifest(hash).await? {
        if latest != ids {
            validate_manifest(&latest)?;
            let batch = db.begin_saved_music_scan_mode(
                uid,
                &latest,
                compute_saved_music_hash(&latest),
                false,
            )?;
            let done = batch.status.indexed as usize;
            publish(batch, done);
            return Err(AppError::msg("Saved music changed during sync; retry"));
        }
    }
    db.prepare_saved_music_soft_finish(uid)?;
    let batch = db.finish_saved_music_scan(uid)?;
    let last_sync_at = batch.status.last_sync_at.clone();
    publish(batch, ids.len());
    Ok(SyncResult {
        changed: true,
        total: ids.len() as i64,
        last_sync_at,
    })
}

#[cfg(test)]
mod tests;
