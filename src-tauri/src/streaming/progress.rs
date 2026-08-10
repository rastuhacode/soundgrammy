use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Notify;

use super::CHUNK_SIZE;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct LoadedRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Serialize)]
pub(super) struct DownloadProgress {
    #[serde(rename = "trackId")]
    track_id: i64,
    received: u64,
    total: u64,
    ranges: Vec<LoadedRange>,
    complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChunkStatus {
    Missing,
    Loading,
    Ready,
}

pub(super) struct ChunkSlot {
    pub(super) status: ChunkStatus,
    pub(super) error: Option<String>,
    pub(super) notify: Arc<Notify>,
}

impl Default for ChunkSlot {
    fn default() -> Self {
        Self {
            status: ChunkStatus::Missing,
            error: None,
            notify: Arc::new(Notify::new()),
        }
    }
}

pub(super) struct StreamState {
    pub(super) chunks: Vec<ChunkSlot>,
    pub(super) received: u64,
    pub(super) terminal_error: Option<String>,
}

pub(super) fn progress_from_state(
    track_id: i64,
    total: u64,
    state: &StreamState,
) -> DownloadProgress {
    DownloadProgress {
        track_id,
        received: state.received,
        total,
        ranges: loaded_ranges(&state.chunks, total),
        complete: state.received == total,
    }
}

pub(super) fn loaded_ranges(chunks: &[ChunkSlot], total: u64) -> Vec<LoadedRange> {
    let mut ranges = Vec::new();
    let mut start = None;

    for (index, slot) in chunks.iter().enumerate() {
        let ready = slot.status == ChunkStatus::Ready;
        match (start, ready) {
            (None, true) => start = Some(index as u64 * CHUNK_SIZE),
            (Some(range_start), false) => {
                ranges.push(LoadedRange {
                    start: range_start,
                    end: index as u64 * CHUNK_SIZE,
                });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(range_start) = start {
        ranges.push(LoadedRange {
            start: range_start,
            end: total,
        });
    }
    ranges
}
