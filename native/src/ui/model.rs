//! Presentation policy shared by the native views and regression tests.
pub(super) use super::library::model::Library;
#[cfg(test)]
use crate::db::{LibraryBatch, LibraryStatus, PlaylistsBundle, TrackListenStats};
use crate::{
    db::Track,
    error::{AppError, AppResult},
    runtime::AppHost,
    state::AppState,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum QueueSaveScope {
    #[default]
    Full,
    FromHere,
    UpNext,
}
pub fn queue_save_ids(queue: &crate::audio::session::Queue, scope: QueueSaveScope) -> Vec<i64> {
    let start = match scope {
        QueueSaveScope::Full => 0,
        QueueSaveScope::FromHere | QueueSaveScope::UpNext => {
            let Ok(cursor) = usize::try_from(queue.cursor) else {
                return vec![];
            };
            if cursor >= queue.tracks.len() {
                return vec![];
            }
            cursor + usize::from(scope == QueueSaveScope::UpNext)
        }
    };
    queue.tracks.iter().skip(start).map(|t| t.id).collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum PlaylistId {
    Custom(i64),
    #[default]
    All,
    Builtin(String),
}
impl PlaylistId {
    pub fn value(&self) -> serde_json::Value {
        match self {
            Self::All => "all".into(),
            Self::Builtin(v) => v.clone().into(),
            Self::Custom(v) => (*v).into(),
        }
    }
    pub fn key(&self) -> String {
        match self {
            Self::All => "all".into(),
            Self::Builtin(v) => v.clone(),
            Self::Custom(v) => v.to_string(),
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "all" => Self::All,
            "liked" | "popular" | "recent" => Self::Builtin(value.into()),
            _ => value
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0)
                .map(Self::Custom)
                .unwrap_or_default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub selected: String,
    pub hidden: HashSet<String>,
    pub sidebar_width: f32,
    pub playlist_sort: String,
    pub reversed: bool,
    pub custom_order: Vec<String>,
    pub keep_awake: bool,
    pub bounce: BounceSettings,
    pub volume: f64,
    pub repeat: crate::audio::session::Repeat,
    pub shuffle: crate::audio::session::Shuffle,
    pub shuffle_mode: crate::audio::session::Mode,
    pub logs_enabled: bool,
    pub history_open: bool,
    pub up_next_open: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            selected: "all".into(),
            hidden: HashSet::new(),
            sidebar_width: 320.,
            playlist_sort: "recency".into(),
            reversed: false,
            custom_order: vec![],
            keep_awake: true,
            bounce: Default::default(),
            volume: if cfg!(target_os = "android") {
                100.
            } else {
                25.
            },
            repeat: Default::default(),
            shuffle: Default::default(),
            shuffle_mode: Default::default(),
            logs_enabled: false,
            history_open: true,
            up_next_open: true,
        }
    }
}
impl Preferences {
    pub fn load(host: &AppHost) -> Self {
        let saved = host
            .state::<AppState>()
            .db
            .get_setting("native_ui_preferences")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok());
        if let Some(saved) = saved {
            return saved;
        }
        let imported = super::preferences::import(host);
        let _ = imported.save(host);
        imported
    }

    pub fn save(&self, host: &AppHost) -> AppResult<()> {
        host.persist_setting("native_ui_preferences", self.clone())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BounceSettings {
    pub enabled: bool,
}

#[derive(Clone)]
pub struct PlaylistItem {
    pub id: PlaylistId,
    pub name: String,
    pub updated: String,
    pub count: usize,
}
#[derive(Clone)]
pub struct Entry {
    pub track: Track,
    pub source_index: usize,
}

pub fn title(track: &Track) -> String {
    track
        .title
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Title".into())
}
pub fn artist(track: &Track) -> String {
    track
        .performer
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Artist".into())
}
pub fn time(seconds: f64) -> String {
    let value = if seconds.is_finite() {
        seconds.max(0.) as u64
    } else {
        0
    };
    format!("{}:{:02}", value / 60, value % 60)
}
pub fn seek(current: f64, duration: f64, offset: f64) -> f64 {
    let current = if current.is_finite() { current } else { 0. };
    if !duration.is_finite() || duration <= 0. {
        current.max(0.)
    } else {
        (current + offset).clamp(0., duration)
    }
}
pub fn login_phone(value: &str) -> AppResult<String> {
    let value = value.trim();
    let digits = value.strip_prefix('+').unwrap_or("");
    if (7..=15).contains(&digits.len())
        && digits
            .as_bytes()
            .first()
            .is_some_and(|v| matches!(v, b'1'..=b'9'))
        && digits.bytes().all(|v| v.is_ascii_digit())
    {
        Ok(value.into())
    } else {
        Err(AppError::msg(
            "Enter a phone number in international format, e.g. +447700900123",
        ))
    }
}
pub fn login_code(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.len() == 5 && value.bytes().all(|v| v.is_ascii_digit()) {
        Ok(value.into())
    } else {
        Err(AppError::msg("Enter the 5-digit login code from Telegram"))
    }
}
pub fn finite_nonnegative(value: &str, label: &str) -> AppResult<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.)
        .ok_or_else(|| AppError::msg(format!("{label} must be a non-negative number")))
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueueItem {
    Header(&'static str, usize),
    Track(usize),
}
pub fn queue_items(
    count: usize,
    cursor: i64,
    history_open: bool,
    next_open: bool,
) -> Vec<QueueItem> {
    let Ok(cursor) = usize::try_from(cursor) else {
        return vec![];
    };
    if cursor >= count {
        return vec![];
    }
    let mut items = vec![];
    if cursor > 0 {
        items.push(QueueItem::Header("History", cursor));
        if history_open {
            items.extend((0..cursor).map(QueueItem::Track));
        }
    }
    items.push(QueueItem::Header("Now playing", 1));
    items.push(QueueItem::Track(cursor));
    if cursor + 1 < count {
        items.push(QueueItem::Header("Up next", count - cursor - 1));
        if next_open {
            items.extend((cursor + 1..count).map(QueueItem::Track));
        }
    }
    items
}
pub fn volume_wheel(delta: f64, lines: bool, remainder: f64) -> (i8, f64) {
    if !delta.is_finite() || delta == 0. {
        return (0, remainder);
    }
    let direction = delta.signum();
    if lines || delta.abs() >= 40. {
        return (if direction > 0. { -1 } else { 1 }, 0.);
    }
    let value = if remainder == 0. || remainder.signum() == direction {
        remainder
    } else {
        0.
    } + delta;
    if value.abs() < 40. {
        (0, value)
    } else {
        (if direction > 0. { -1 } else { 1 }, value - direction * 40.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn track(id: i64) -> Track {
        serde_json::from_value(serde_json::json!({"id":id,"title":format!("Track {id}"),"performer":"Artist","duration":180,"mime_type":"audio/mpeg","file_size":42})).unwrap()
    }
    #[test]
    fn queue_save_scopes_preserve_positions_and_idle_policy() {
        use QueueSaveScope::*;
        let mut queue = crate::audio::session::Queue {
            tracks: vec![track(1), track(2), track(1), track(3)],
            cursor: 1,
            ..Default::default()
        };
        assert_eq!(queue_save_ids(&queue, Full), vec![1, 2, 1, 3]);
        assert_eq!(queue_save_ids(&queue, FromHere), vec![2, 1, 3]);
        assert_eq!(queue_save_ids(&queue, UpNext), vec![1, 3]);
        queue.cursor = 3;
        assert!(queue_save_ids(&queue, UpNext).is_empty());
        for cursor in [-1, 4] {
            queue.cursor = cursor;
            assert_eq!(queue_save_ids(&queue, Full), vec![1, 2, 1, 3]);
            assert!(queue_save_ids(&queue, FromHere).is_empty());
            assert!(queue_save_ids(&queue, UpNext).is_empty());
        }
    }
    fn library() -> Library {
        let mut library = Library::default();
        library.tracks = vec![track(1), track(2), track(3)];
        library.playlists = Some(PlaylistsBundle {
            liked: crate::db::LikedPlaylist {
                id: 10,
                track_ids: vec![1, 2, 1],
                updated_at: String::new(),
            },
            custom: vec![crate::db::CustomPlaylistSummary {
                id: 20,
                name: "Mix".into(),
                track_ids: vec![1, 99, 2, 1],
                updated_at: String::new(),
            }],
        });
        library
    }
    #[test]
    fn hydration_keeps_duplicate_selection_only_for_the_same_ordered_membership() {
        let mut current = library();
        current.select(PlaylistId::Custom(20));
        current.selection = [0, 3].into_iter().collect();
        current.selection_anchor = Some(3);
        current.search = "Track".into();
        current.replace_hydration(library());
        assert_eq!(current.selection, [0, 3].into_iter().collect());
        assert_eq!(current.selection_anchor, Some(3));
        assert_eq!(current.search, "Track");
        let mut reordered = library();
        reordered.playlists.as_mut().unwrap().custom[0]
            .track_ids
            .swap(1, 3);
        current.replace_hydration(reordered);
        assert!(current.selection.is_empty());
        assert_eq!(current.selection_anchor, None);
    }
    #[test]
    fn failed_optional_reads_keep_the_library_and_last_known_domain_values() {
        let mut current = library();
        current.select(PlaylistId::Custom(20));
        current.cached.insert(1);
        current.stats.insert(1, stats(1, 0.5, Some(10)));
        let mut incoming = Library::default();
        incoming.tracks = vec![track(1), track(2), track(3)];
        incoming.warnings = vec!["Could not load playlists".into()];
        current.replace_hydration(incoming);
        assert_eq!(current.tracks.len(), 3);
        assert_eq!(current.selected, PlaylistId::Custom(20));
        assert_eq!(current.membership(), vec![1, 99, 2, 1]);
        assert!(current.cached.contains(&1));
        assert_eq!(current.stats[&1].last_played_at_ms, Some(10));
        assert_eq!(current.warnings, vec!["Could not load playlists"]);
    }
    fn stats(id: i64, likeness: f64, last: Option<i64>) -> TrackListenStats {
        TrackListenStats {
            track_id: id,
            starts: 1,
            qualified_plays: 1,
            completes: 0,
            early_skips: 0,
            total_listened_ms: 1000,
            first_played_at_ms: Some(1),
            last_played_at_ms: last,
            likeness,
        }
    }
    #[test]
    fn duplicate_memberships_keep_their_original_positions_across_missing_metadata() {
        let mut l = library();
        l.select(PlaylistId::Custom(20));
        let entries = l.entries();
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.track.id, e.source_index))
                .collect::<Vec<_>>(),
            vec![(1, 0), (2, 2), (1, 3)]
        );
        l.selection.insert(3);
        assert_eq!(l.selected_entries()[0].source_index, 3);
        l.search = "track 1".into();
        assert_eq!(
            l.entries()
                .iter()
                .map(|e| e.source_index)
                .collect::<Vec<_>>(),
            vec![0, 3]
        );
    }
    #[test]
    fn switching_playlists_clears_positional_selection_search_and_sort() {
        let mut l = library();
        l.selection.insert(0);
        l.search = "foo".into();
        l.sort = "artist".into();
        l.descending = true;
        l.select(PlaylistId::Custom(20));
        assert!(l.selection.is_empty());
        assert!(l.search.is_empty());
        assert!(l.sort.is_empty());
        assert!(!l.descending);
    }
    #[test]
    fn smart_playlists_use_only_available_library_tracks_and_stable_id_ties() {
        let mut l = library();
        l.stats = vec![
            stats(1, 20., None),
            stats(2, 20., Some(10)),
            stats(3, 10., Some(20)),
            stats(99, 100., Some(30)),
        ]
        .into_iter()
        .map(|s| (s.track_id, s))
        .collect();
        l.select(PlaylistId::Builtin("popular".into()));
        assert_eq!(l.membership(), vec![1, 2, 3]);
        l.select(PlaylistId::Builtin("recent".into()));
        assert_eq!(l.membership(), vec![3, 2, 1]);
    }
    #[test]
    fn sorting_and_search_do_not_change_backend_membership_positions() {
        let mut l = library();
        l.select(PlaylistId::Builtin("liked".into()));
        l.sort = "title".into();
        l.descending = true;
        assert_eq!(
            l.entries()
                .iter()
                .map(|e| (e.track.id, e.source_index))
                .collect::<Vec<_>>(),
            vec![(2, 1), (1, 0), (1, 2)]
        );
        assert_eq!(l.membership(), vec![1, 2, 1]);
    }
    #[test]
    fn missing_artist_and_duration_sort_like_develop() {
        let mut library = library();
        library.tracks[0].performer = None;
        library.tracks[0].duration = None;
        for column in ["artist", "duration"] {
            library.sort = column.into();
            library.descending = false;
            assert_eq!(library.entries().last().unwrap().track.id, 1);
            library.descending = true;
            assert_eq!(library.entries().first().unwrap().track.id, 1);
        }
    }
    #[test]
    fn sidebar_recency_uses_library_dates_and_ignores_removed_tracks() {
        let mut library = library();
        for track in &mut library.tracks {
            track.created_at = "2024-01-01".into();
        }
        library.playlists.as_mut().unwrap().custom[0].updated_at = "2025-01-01".into();
        library
            .stats
            .insert(99, stats(99, 100., Some(9_999_999_999_999)));
        let items = library.items(&Preferences::default());
        assert_eq!(items[0].id, PlaylistId::Custom(20));
        assert_eq!(
            items
                .iter()
                .find(|p| p.id == PlaylistId::All)
                .unwrap()
                .updated,
            "2024-01-01"
        );
        assert!(items
            .iter()
            .find(|p| p.id == PlaylistId::Builtin("recent".into()))
            .unwrap()
            .updated
            .is_empty());
    }
    #[test]
    fn all_tracks_cannot_be_hidden_and_custom_sort_keeps_all_items() {
        let l = library();
        let p = Preferences {
            hidden: ["all".into(), "liked".into()].into_iter().collect(),
            playlist_sort: "custom".into(),
            custom_order: vec!["20".into(), "all".into()],
            ..Default::default()
        };
        let items = l.items(&p);
        assert_eq!(items[0].id, PlaylistId::Custom(20));
        assert!(items.iter().any(|p| p.id == PlaylistId::All));
        assert!(!items
            .iter()
            .any(|p| p.id == PlaylistId::Builtin("liked".into())));
    }
    #[test]
    fn notification_batches_reject_other_accounts_and_older_revisions() {
        let mut l = library();
        l.status = Some(LibraryStatus {
            tg_user_id: 1,
            revision: 10,
            indexed: 3,
            total: 3,
            complete: true,
            blocking: false,
            processed: 3,
            last_sync_at: None,
        });
        let mut status = l.status.clone().unwrap();
        status.revision = 9;
        let mut batch = LibraryBatch {
            tracks: vec![],
            removed_track_ids: vec![1],
            status,
        };
        assert!(!l.apply_batch(&batch));
        assert_eq!(l.tracks.len(), 3);
        batch.status.revision = 11;
        batch.status.tg_user_id = 2;
        assert!(!l.apply_batch(&batch));
        batch.status.tg_user_id = 1;
        assert!(l.apply_batch(&batch));
        assert_eq!(l.tracks.len(), 2);
    }
    #[test]
    fn batches_strip_telegram_descriptors_and_clear_invalidated_selection() {
        let mut l = library();
        l.selection.insert(1);
        let mut added = track(4);
        added.mtproto_document = Some("backend descriptor".into());
        let status = LibraryStatus {
            tg_user_id: 1,
            revision: 1,
            indexed: 4,
            total: 4,
            complete: true,
            blocking: false,
            processed: 4,
            last_sync_at: None,
        };
        l.apply_batch(&LibraryBatch {
            tracks: vec![crate::db::PositionedTrack {
                track: added,
                position: Some(3),
            }],
            removed_track_ids: vec![],
            status,
        });
        assert!(l.selection.is_empty());
        assert!(l.tracks.iter().all(|t| t.mtproto_document.is_none()));
    }
    #[test]
    fn seeks_are_finite_and_bounded_and_unknown_duration_does_not_jump() {
        for (current, duration, offset, expected) in [
            (0., 100., -5., 0.),
            (98., 100., 5., 100.),
            (20., 0., 5., 20.),
            (20., f64::NAN, 5., 20.),
            (f64::NAN, 100., 5., 5.),
        ] {
            assert_eq!(seek(current, duration, offset), expected);
        }
    }
    #[test]
    fn invalid_numeric_settings_cannot_reach_native_services() {
        for value in ["", "NaN", "inf", "-1", "foo"] {
            assert!(finite_nonnegative(value, "Limit").is_err());
        }
        for value in ["0", "0.5", "30"] {
            assert!(finite_nonnegative(value, "Limit").is_ok());
        }
    }
    #[test]
    fn playlist_identity_parsing_rejects_invalid_or_nonpositive_custom_ids() {
        for value in ["", "0", "-1", "wat", "1.5"] {
            assert_eq!(PlaylistId::parse(value), PlaylistId::All);
        }
        assert_eq!(PlaylistId::parse("20"), PlaylistId::Custom(20));
        assert_eq!(
            PlaylistId::parse("liked"),
            PlaylistId::Builtin("liked".into())
        );
    }
    #[test]
    fn format_time_handles_nonfinite_and_long_tracks() {
        for (value, expected) in [
            (f64::NAN, "0:00"),
            (-1., "0:00"),
            (59., "0:59"),
            (60., "1:00"),
            (3661., "61:01"),
        ] {
            assert_eq!(time(value), expected);
        }
    }
    #[test]
    fn native_preferences_roundtrip_preserves_every_user_option() {
        let mut p = Preferences {
            selected: "20".into(),
            ..Default::default()
        };
        p.bounce.enabled = true;
        p.keep_awake = true;
        p.custom_order = vec!["20".into(), "all".into()];
        p.hidden.insert("recent".into());
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(restored.selected, p.selected);
        assert_eq!(restored.custom_order, p.custom_order);
        assert_eq!(restored.hidden, p.hidden);
        assert!(restored.bounce.enabled && restored.keep_awake);
    }
    #[test]
    fn shift_ranges_follow_filtered_display_order_and_update_anchor() {
        let mut l = library();
        l.select(PlaylistId::Custom(20));
        l.select_entry(0, false);
        l.select_entry(3, true);
        assert_eq!(l.selection.len(), 3);
        assert_eq!(l.selection_anchor, Some(3));
        l.clear_selection();
        l.search = "track 1".into();
        l.select_entry(0, false);
        l.select_entry(3, true);
        assert_eq!(l.selection, [0, 3].into_iter().collect());
        l.search = "track 2".into();
        l.select_entry(2, true);
        assert!(l.selection.contains(&2));
    }
    #[test]
    fn multiple_sort_columns_preserve_membership_ties_and_nulls_last() {
        let mut l = library();
        l.tracks[0].title = Some("Same".into());
        l.tracks[1].title = Some("Same".into());
        l.tracks[0].performer = Some("Z".into());
        l.tracks[1].performer = Some("A".into());
        l.tracks[2].title = None;
        l.toggle_sort("title", false);
        l.toggle_sort("artist", true);
        assert_eq!(
            l.entries().iter().map(|e| e.track.id).collect::<Vec<_>>(),
            vec![2, 1, 3]
        );
    }
    #[test]
    fn now_playing_distinguishes_duplicate_memberships_even_after_queue_edits() {
        let mut l = library();
        l.select(PlaylistId::Builtin("liked".into()));
        let mut session = crate::audio::session::Session::default();
        session.queue = crate::audio::session::Queue {
            tracks: vec![track(1), track(2), track(1)],
            cursor: 2,
            ..Default::default()
        };
        let snapshot = crate::audio::Snapshot {
            track_id: Some(1),
            player: Some(std::sync::Arc::new(session)),
            ..Default::default()
        };
        assert_eq!(l.playing_source_index(&snapshot), Some(2));
    }
    #[test]
    fn queue_sections_omit_only_collapsed_memberships() {
        assert_eq!(
            queue_items(4, 1, false, false),
            vec![
                QueueItem::Header("History", 1),
                QueueItem::Header("Now playing", 1),
                QueueItem::Track(1),
                QueueItem::Header("Up next", 2)
            ]
        );
        assert_eq!(
            queue_items(4, 1, true, true)
                .iter()
                .filter(|i| matches!(i, QueueItem::Track(_)))
                .count(),
            4
        );
        assert!(queue_items(0, -1, true, true).is_empty());
        assert!(queue_items(2, 5, true, true).is_empty());
    }
    #[test]
    fn trackpad_volume_is_deliberate_and_direction_changes_reset_remainder() {
        assert_eq!(volume_wheel(10., false, 0.), (0, 10.));
        assert_eq!(volume_wheel(35., false, 10.), (-1, 5.));
        assert_eq!(volume_wheel(-10., false, 10.), (0, -10.));
        assert_eq!(volume_wheel(-1., true, 0.), (1, 0.));
        assert_eq!(volume_wheel(f64::NAN, false, 12.), (0, 12.));
    }
    #[test]
    fn login_validation_matches_international_phone_and_five_digit_code_contracts() {
        for value in ["+447700900123", " +1234567 "] {
            assert!(login_phone(value).is_ok());
        }
        for value in [
            "1234567",
            "+01234567",
            "+123456",
            "+1234567890123456",
            "+1234-567",
            "+1234abc",
        ] {
            assert!(login_phone(value).is_err(), "{value}");
        }
        for value in ["12345", " 01234 "] {
            assert!(login_code(value).is_ok());
        }
        for value in ["1234", "123456", "abcde", "１２３４５"] {
            assert!(login_code(value).is_err());
        }
    }
}
