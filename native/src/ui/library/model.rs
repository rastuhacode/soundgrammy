//! Observable library data and pure projection/selection policy.
use super::super::model::{artist, title, Entry, PlaylistId, PlaylistItem, Preferences};
use crate::{
    commands,
    db::{LibraryBatch, LibraryStatus, PlaylistsBundle, Track, TrackListenStats},
    error::AppResult,
    runtime::AppHost,
    state::AppState,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
fn nullable_cmp<T: Ord>(a: Option<T>, b: Option<T>) -> std::cmp::Ordering {
    match (a, b) {
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (a, b) => a.cmp(&b),
    }
}

#[derive(Clone)]
pub(in crate::ui) struct TrackList {
    pub membership: Arc<[i64]>,
    pub entries: Arc<[Entry]>,
}

#[derive(PartialEq)]
struct TrackListKey {
    playlist: PlaylistId,
    search: String,
    sort: String,
    descending: bool,
    additional_sort: Vec<(String, bool)>,
}

#[derive(Default)]
pub struct Library {
    pub warnings: Vec<String>,
    pub stats_loaded: bool,
    pub cache_loaded: bool,
    pub tracks: Vec<Track>,
    pub positions: HashMap<i64, i64>,
    pub status: Option<LibraryStatus>,
    pub playlists: Option<PlaylistsBundle>,
    pub stats: HashMap<i64, TrackListenStats>,
    pub cached: HashSet<i64>,
    pub selected: PlaylistId,
    pub selection: HashSet<usize>,
    pub selection_anchor: Option<usize>,
    pub search: String,
    pub sort: String,
    pub descending: bool,
    pub additional_sort: Vec<(String, bool)>,
    track_list: std::cell::RefCell<Option<(TrackListKey, TrackList)>>,
}

impl Library {
    /// Replace one revision-checked data snapshot while keeping view state tied
    /// to the exact same ordered membership. Failed optional domains retain
    /// their last usable values and remain independently retryable.
    pub fn replace_hydration(&mut self, mut incoming: Self) {
        incoming.selected = self.selected.clone();
        if incoming.playlists.is_none() {
            incoming.playlists = self.playlists.clone();
        }
        if !incoming.stats_loaded {
            incoming.stats = self.stats.clone();
        }
        if !incoming.cache_loaded {
            incoming.cached = self.cached.clone();
        }
        if let PlaylistId::Custom(id) = incoming.selected {
            if !incoming
                .playlists
                .as_ref()
                .is_some_and(|p| p.custom.iter().any(|p| p.id == id))
            {
                incoming.selected = PlaylistId::All;
            }
        }
        if incoming.selected == self.selected && incoming.membership() == self.membership() {
            incoming.selection = self.selection.clone();
            incoming.selection_anchor = self.selection_anchor;
        }
        incoming.search = self.search.clone();
        incoming.sort = self.sort.clone();
        incoming.descending = self.descending;
        incoming.additional_sort = self.additional_sort.clone();
        *self = incoming;
    }

    pub fn select(&mut self, id: PlaylistId) {
        if self.selected != id {
            self.selected = id;
            self.selection.clear();
            self.selection_anchor = None;
            self.search.clear();
            self.sort.clear();
            self.additional_sort.clear();
            self.descending = false;
        }
    }
    pub fn membership(&self) -> Vec<i64> {
        match &self.selected {
            PlaylistId::All => self.tracks.iter().map(|t| t.id).collect(),
            PlaylistId::Builtin(id) if id == "liked" => self
                .playlists
                .as_ref()
                .map(|p| p.liked.track_ids.clone())
                .unwrap_or_default(),
            PlaylistId::Custom(id) => self
                .playlists
                .as_ref()
                .and_then(|p| p.custom.iter().find(|p| p.id == *id))
                .map(|p| p.track_ids.clone())
                .unwrap_or_default(),
            PlaylistId::Builtin(id) => {
                let mut tracks: Vec<_> = self
                    .tracks
                    .iter()
                    .filter(|t| self.stats.contains_key(&t.id))
                    .collect();
                tracks.sort_by(|a, b| {
                    let a_stat = &self.stats[&a.id];
                    let b_stat = &self.stats[&b.id];
                    let order = if id == "popular" {
                        b_stat.likeness.total_cmp(&a_stat.likeness)
                    } else {
                        b_stat.last_played_at_ms.cmp(&a_stat.last_played_at_ms)
                    };
                    order.then(a.id.cmp(&b.id))
                });
                tracks.into_iter().map(|t| t.id).collect()
            }
        }
    }
    pub(in crate::ui) fn invalidate_track_list(&mut self) {
        self.track_list.get_mut().take();
    }

    pub(in crate::ui) fn track_list(&self) -> TrackList {
        let mut cached = self.track_list.borrow_mut();
        if let Some((key, list)) = cached.as_ref() {
            if key.playlist == self.selected
                && key.search == self.search
                && key.sort == self.sort
                && key.descending == self.descending
                && key.additional_sort == self.additional_sort
            {
                return list.clone();
            }
        }

        let membership = self.membership();
        let entries = self.entries_in(&membership);
        let list = TrackList {
            membership: membership.into(),
            entries: entries.into(),
        };
        *cached = Some((
            TrackListKey {
                playlist: self.selected.clone(),
                search: self.search.clone(),
                sort: self.sort.clone(),
                descending: self.descending,
                additional_sort: self.additional_sort.clone(),
            },
            list.clone(),
        ));
        list
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.track_list().entries.to_vec()
    }

    fn entries_in(&self, membership: &[i64]) -> Vec<Entry> {
        let by_id: HashMap<_, _> = self.tracks.iter().map(|t| (t.id, t)).collect();
        let query = self.search.to_lowercase();
        let mut entries: Vec<_> = membership
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(source_index, id)| {
                let track = by_id.get(&id)?;
                if !query.is_empty()
                    && !format!("{} {}", title(track), artist(track))
                        .to_lowercase()
                        .contains(&query)
                {
                    return None;
                }
                Some(Entry {
                    track: (*track).clone(),
                    source_index,
                })
            })
            .collect();
        if !self.sort.is_empty() {
            entries.sort_by(|a, b| {
                for (column, descending) in std::iter::once((&self.sort, &self.descending))
                    .chain(self.additional_sort.iter().map(|(a, b)| (a, b)))
                {
                    let order = match column.as_str() {
                        "artist" => nullable_cmp(
                            a.track.performer.as_ref().map(|v| v.to_lowercase()),
                            b.track.performer.as_ref().map(|v| v.to_lowercase()),
                        ),
                        "duration" => nullable_cmp(a.track.duration, b.track.duration),
                        _ => match (&a.track.title, &b.track.title) {
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (a, b) => a
                                .as_ref()
                                .map(|v| v.to_lowercase())
                                .cmp(&b.as_ref().map(|v| v.to_lowercase())),
                        },
                    };
                    let order = if *descending { order.reverse() } else { order };
                    if !order.is_eq() {
                        return order;
                    }
                }
                a.source_index.cmp(&b.source_index)
            });
        }
        entries
    }
    pub fn toggle_sort(&mut self, column: &str, extend: bool) {
        if !extend {
            if self.sort == column && self.descending {
                self.sort.clear();
                self.descending = false;
            } else {
                self.descending = self.sort == column;
                self.sort = column.into();
            }
            self.additional_sort.clear();
        } else if self.sort.is_empty() {
            self.sort = column.into();
            self.descending = false;
        } else if self.sort == column {
            self.descending = !self.descending;
        } else if let Some(index) = self
            .additional_sort
            .iter()
            .position(|(key, _)| key == column)
        {
            if self.additional_sort[index].1 {
                self.additional_sort.remove(index);
            } else {
                self.additional_sort[index].1 = true;
            }
        } else {
            self.additional_sort.push((column.into(), false));
        }
    }
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selection_anchor = None;
    }
    pub fn playing_source_index(&self, snapshot: &crate::audio::Snapshot) -> Option<usize> {
        let current = snapshot.track_id?;
        let queue = &snapshot.player.as_ref()?.queue;
        let cursor = usize::try_from(queue.cursor).ok()?;
        let list = self.track_list();
        let membership = &list.membership;
        let playlist_id = if self.selected == PlaylistId::Builtin("liked".into()) {
            self.playlists
                .as_ref()
                .map(|p| serde_json::json!(p.liked.id))
                .unwrap_or_else(|| self.selected.value())
        } else {
            self.selected.value()
        };
        if queue
            .source
            .as_ref()
            .is_some_and(|s| s.playlist_id == playlist_id)
        {
            if let Some(index) = queue
                .source_indices
                .as_ref()
                .and_then(|v| v.get(cursor))
                .copied()
                .filter(|index| membership.get(*index) == Some(&current))
            {
                return Some(index);
            }
            if queue
                .tracks
                .iter()
                .map(|t| t.id)
                .eq(membership.iter().copied())
            {
                return (cursor < membership.len()).then_some(cursor);
            }
        }
        if queue.tracks.get(cursor).is_some_and(|t| t.id == current) {
            let occurrence = queue.tracks[..=cursor]
                .iter()
                .filter(|t| t.id == current)
                .count()
                - 1;
            if let Some((index, _)) = membership
                .iter()
                .enumerate()
                .filter(|(_, id)| **id == current)
                .nth(occurrence)
            {
                return Some(index);
            }
        }
        membership.iter().position(|id| *id == current)
    }
    pub fn select_entry(&mut self, position: usize, extend: bool) {
        let order = self.entries();
        let anchor = self
            .selection_anchor
            .and_then(|anchor| order.iter().position(|e| e.source_index == anchor));
        let target = order.iter().position(|e| e.source_index == position);
        if extend {
            if let (Some(a), Some(b)) = (anchor, target) {
                for entry in &order[a.min(b)..=a.max(b)] {
                    self.selection.insert(entry.source_index);
                }
            } else {
                self.selection.insert(position);
            }
            self.selection_anchor = Some(position);
        } else if self.selection.insert(position) {
            self.selection_anchor = Some(position);
        } else {
            self.selection.remove(&position);
            if self.selection_anchor == Some(position) {
                self.selection_anchor = None;
            }
        }
    }
    pub fn selected_entries(&self) -> Vec<Entry> {
        if self.selection.is_empty() {
            return Vec::new();
        }
        self.track_list()
            .entries
            .iter()
            .filter(|e| self.selection.contains(&e.source_index))
            .cloned()
            .collect()
    }
    pub fn reorderable(&self) -> bool {
        matches!(&self.selected, PlaylistId::Custom(_))
            || self.selected == PlaylistId::Builtin("liked".into())
    }
    pub fn name(&self) -> String {
        match &self.selected {
            PlaylistId::All => "All tracks".into(),
            PlaylistId::Builtin(v) => match v.as_str() {
                "liked" => "Liked",
                "popular" => "Popular",
                _ => "Recent",
            }
            .into(),
            PlaylistId::Custom(id) => self
                .playlists
                .as_ref()
                .and_then(|p| p.custom.iter().find(|p| p.id == *id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Playlist".into()),
        }
    }
    pub fn liked(&self, id: i64) -> bool {
        self.playlists
            .as_ref()
            .is_some_and(|p| p.liked.track_ids.contains(&id))
    }
    pub fn items(&self, prefs: &Preferences) -> Vec<PlaylistItem> {
        let mut items = vec![PlaylistItem {
            id: PlaylistId::All,
            name: "All tracks".into(),
            updated: self
                .tracks
                .iter()
                .map(|t| &t.created_at)
                .max()
                .cloned()
                .unwrap_or_default(),
            count: self.tracks.len(),
        }];
        for (id, name) in [
            ("liked", "Liked"),
            ("popular", "Popular"),
            ("recent", "Recent"),
        ] {
            if prefs.hidden.contains(id) {
                continue;
            }
            let count = if id == "liked" {
                self.playlists
                    .as_ref()
                    .map(|p| p.liked.track_ids.len())
                    .unwrap_or(0)
            } else {
                self.tracks
                    .iter()
                    .filter(|t| self.stats.contains_key(&t.id))
                    .count()
            };
            let updated = if id == "liked" {
                self.playlists
                    .as_ref()
                    .map(|p| p.liked.updated_at.clone())
                    .unwrap_or_default()
            } else {
                self.tracks
                    .iter()
                    .filter_map(|t| self.stats.get(&t.id))
                    .filter_map(|s| s.last_played_at_ms)
                    .max()
                    .map(crate::time::iso_utc)
                    .unwrap_or_default()
            };
            items.push(PlaylistItem {
                id: PlaylistId::Builtin(id.into()),
                name: name.into(),
                updated,
                count,
            });
        }
        if let Some(playlists) = &self.playlists {
            items.extend(playlists.custom.iter().map(|p| PlaylistItem {
                id: PlaylistId::Custom(p.id),
                name: p.name.clone(),
                updated: p.updated_at.clone(),
                count: p.track_ids.len(),
            }));
        }
        items.sort_by(|a, b| match prefs.playlist_sort.as_str() {
            "alphabetical" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            "custom" => prefs
                .custom_order
                .iter()
                .position(|v| v == &a.id.key())
                .unwrap_or(usize::MAX)
                .cmp(
                    &prefs
                        .custom_order
                        .iter()
                        .position(|v| v == &b.id.key())
                        .unwrap_or(usize::MAX),
                ),
            _ => b.updated.cmp(&a.updated),
        });
        if prefs.reversed && prefs.playlist_sort != "custom" {
            items.reverse();
        }
        items
    }
    pub fn apply_batch(&mut self, batch: &LibraryBatch) -> bool {
        if self.status.as_ref().is_some_and(|s| {
            s.tg_user_id != batch.status.tg_user_id || s.revision > batch.status.revision
        }) {
            return false;
        }
        let old_membership = self.membership();
        self.tracks
            .retain(|t| !batch.removed_track_ids.contains(&t.id));
        for id in &batch.removed_track_ids {
            self.positions.remove(id);
            self.stats.remove(id);
            self.cached.remove(id);
        }
        for item in &batch.tracks {
            let mut track = item.track.clone();
            track.mtproto_document = None;
            if let Some(old) = self.tracks.iter_mut().find(|t| t.id == track.id) {
                *old = track;
            } else {
                self.tracks.push(track);
            }
            if let Some(position) = item.position {
                self.positions.insert(item.track.id, position);
            }
        }
        self.tracks
            .sort_by_key(|t| self.positions.get(&t.id).copied().unwrap_or(i64::MAX));
        self.status = Some(batch.status.clone());
        self.invalidate_track_list();
        if old_membership != self.membership() {
            self.selection.clear();
            self.selection_anchor = None;
        }
        true
    }
}

pub async fn hydrate(host: AppHost) -> AppResult<Library> {
    let state = host.state::<AppState>();
    let mut result = Library::default();
    // A concurrent sync invalidates the revision. Retry a bounded number of full
    // reads rather than combine pages from different membership snapshots.
    for attempt in 0..3 {
        result.tracks.clear();
        result.positions.clear();
        result.status = None;
        let mut offset = 0;
        let mut revision = None;
        let mut restart = false;
        loop {
            let page = match commands::list_tracks_page(&state, offset, 500, revision).await {
                Ok(page) => page,
                Err(_) if attempt < 2 => {
                    restart = true;
                    break;
                }
                Err(error) => return Err(error),
            };
            revision = Some(page.status.revision);
            for entry in page.tracks {
                let mut track = entry.track;
                track.mtproto_document = None;
                if let Some(p) = entry.position {
                    result.positions.insert(track.id, p);
                }
                result.tracks.push(track);
            }
            result.status = Some(page.status);
            if let Some(next) = page.next_offset {
                offset = next;
            } else {
                break;
            }
        }
        if !restart {
            break;
        }
    }
    match commands::playlists::list_playlists(&state).await {
        Ok(playlists) => result.playlists = Some(playlists),
        Err(error) => result
            .warnings
            .push(format!("Could not load playlists: {error}")),
    }
    match commands::listen_stats::list_listen_stats(&state).await {
        Ok(stats) => {
            result.stats = stats.into_iter().map(|s| (s.track_id, s)).collect();
            result.stats_loaded = true;
        }
        Err(error) => result
            .warnings
            .push(format!("Could not load listening statistics: {error}")),
    }
    match commands::get_cache_status(&state).await {
        Ok(cached) => {
            result.cached = cached.into_iter().collect();
            result.cache_loaded = true;
        }
        Err(error) => result
            .warnings
            .push(format!("Could not load cache status: {error}")),
    }
    Ok(result)
}
