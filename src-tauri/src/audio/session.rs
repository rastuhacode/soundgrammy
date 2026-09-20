//! Authoritative queue policy. No WebView, device, or timer is needed to advance it.
use crate::db::Track;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub r#type: String,
    pub playlist_id: serde_json::Value,
    pub name: String,
    pub track_ids: Vec<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub track: Track,
    pub source_index: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    pub source: Option<Source>,
    pub tracks: Vec<Track>,
    pub cursor: i64,
    pub source_indices: Option<Vec<usize>>,
    pub base_entries: Option<Vec<Entry>>,
}
impl Default for Queue {
    fn default() -> Self {
        Self {
            source: None,
            tracks: vec![],
            cursor: -1,
            source_indices: None,
            base_entries: None,
        }
    }
}
impl Queue {
    pub fn current(&self) -> Option<&Track> {
        self.tracks.get(self.cursor as usize)
    }
    fn normalize(&mut self) {
        self.cursor = if self.tracks.is_empty() {
            -1
        } else {
            self.cursor.clamp(0, self.tracks.len() as i64 - 1)
        };
    }
    fn edited(&mut self) {
        self.source = None;
        self.source_indices = None;
        self.base_entries = None;
    }
    fn entries(&self) -> Vec<Entry> {
        self.tracks
            .iter()
            .enumerate()
            .map(|(i, t)| Entry {
                track: t.clone(),
                source_index: self
                    .source_indices
                    .as_ref()
                    .and_then(|v| v.get(i))
                    .copied()
                    .unwrap_or(i),
            })
            .collect()
    }
    fn install_entries(&mut self, entries: Vec<Entry>, pin: Option<usize>) {
        self.cursor = pin
            .and_then(|p| entries.iter().position(|e| e.source_index == p))
            .unwrap_or(0) as i64;
        self.source_indices = Some(entries.iter().map(|e| e.source_index).collect());
        self.tracks = entries.into_iter().map(|e| e.track).collect();
        self.normalize();
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    None,
    One,
    All,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shuffle {
    #[default]
    Off,
    On,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Random,
    Variety,
    Rediscover,
    Smart,
    Fresh,
    Duration,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub repeat: Repeat,
    pub shuffle: Shuffle,
    pub mode: Mode,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub revision: u64,
    pub queue: Queue,
    pub is_playing: bool,
    pub attempt: u64,
    pub end_reason: &'static str,
    pub preferences: Preferences,
    #[serde(skip)]
    pub completed: bool,
    #[serde(skip)]
    preferences_initialized: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    Attach {
        preferences: Option<Preferences>,
    },
    SetQueue {
        queue: Queue,
        play: bool,
    },
    PlayPlaylist {
        queue: Queue,
        shuffle: Option<Shuffle>,
        #[serde(default)]
        toggle_if_current: bool,
        /// A selected row is pinned; toolbar shuffle has no selected start.
        #[serde(default)]
        pin_start: bool,
    },
    PlayTrack {
        track: Track,
    },
    Clear,
    ClearUpNext,
    Insert {
        tracks: Vec<Track>,
        next: bool,
    },
    Reorder {
        from_index: i64,
        to_index: i64,
        queue_revision: u64,
    },
    Remove {
        indices: Vec<i64>,
        queue_revision: u64,
    },
    Jump {
        index: i64,
        queue_revision: u64,
    },
    Realign {
        playlist_id: serde_json::Value,
        tracks: Vec<Track>,
        move_indices: Option<[i64; 2]>,
    },
    Refresh {
        tracks: Vec<Track>,
    },
    Playing {
        playing: bool,
    },
    Toggle,
    Next,
    Previous {
        restart: bool,
    },
    Repeat {
        repeat: Repeat,
    },
    ToggleRepeat,
    ToggleShuffle,
    Shuffle {
        shuffle: Shuffle,
    },
    ShuffleMode {
        mode: Mode,
    },
}
impl Command {
    pub fn saves_preferences(&self) -> bool {
        matches!(
            self,
            Self::Attach { .. }
                | Self::Repeat { .. }
                | Self::ToggleRepeat
                | Self::ToggleShuffle
                | Self::Shuffle { .. }
                | Self::ShuffleMode { .. }
                | Self::PlayPlaylist {
                    shuffle: Some(_),
                    ..
                }
        )
    }
}
#[derive(Debug, PartialEq)]
pub enum Effect {
    None,
    Load,
    Unload,
    Play,
    Pause,
    Seek(f64),
}
pub fn remap(cursor: i64, from: i64, to: i64) -> i64 {
    if cursor == from {
        to
    } else if from < to && cursor > from && cursor <= to {
        cursor - 1
    } else if from > to && cursor >= to && cursor < from {
        cursor + 1
    } else {
        cursor
    }
}
impl Session {
    pub fn with_preferences(stored: Option<&str>) -> Self {
        let preferences = stored.and_then(|raw| serde_json::from_str(raw).ok());
        Self {
            preferences_initialized: preferences.is_some(),
            preferences: preferences.unwrap_or_default(),
            ..Default::default()
        }
    }
    fn start(&mut self, reason: &'static str) -> Effect {
        self.attempt += 1;
        self.end_reason = reason;
        self.completed = false;
        if self.queue.current().is_none() {
            self.is_playing = false;
            Effect::Unload
        } else {
            Effect::Load
        }
    }
    pub fn complete(&mut self, attempt: u64) -> Effect {
        if attempt != self.attempt
            || self.completed
            || !self.is_playing
            || self.queue.current().is_none()
        {
            return Effect::None;
        }
        self.completed = true;
        self.end_reason = "completed";
        self.revision += 1;
        if self.preferences.repeat == Repeat::One {
            return self.start("completed");
        }
        if self.queue.cursor + 1 < self.queue.tracks.len() as i64 {
            self.queue.cursor += 1;
            self.start("completed")
        } else if self.preferences.repeat == Repeat::All {
            self.queue.cursor = 0;
            self.start("completed")
        } else {
            self.is_playing = false;
            Effect::None
        }
    }
    fn playing(&mut self, playing: bool) -> Effect {
        if self.queue.current().is_none() {
            return Effect::None;
        }
        self.is_playing = playing;
        if playing && self.completed {
            self.start("completed")
        } else if playing {
            Effect::Play
        } else {
            Effect::Pause
        }
    }
    fn shuffle(
        &mut self,
        shuffle: Shuffle,
        pin_current: bool,
        permute: &mut impl FnMut(Vec<Entry>, Mode) -> Vec<Entry>,
    ) {
        self.preferences.shuffle = shuffle;
        if self.queue.current().is_none() {
            return;
        }
        let sourced = self.queue.source.is_some();
        if !sourced && shuffle == Shuffle::Off {
            return;
        }
        let pin = if !pin_current {
            None
        } else if sourced {
            self.queue
                .source_indices
                .as_ref()
                .and_then(|v| v.get(self.queue.cursor as usize))
                .copied()
        } else {
            Some(self.queue.cursor as usize)
        };
        let base = if sourced {
            self.queue
                .base_entries
                .clone()
                .unwrap_or_else(|| self.queue.entries())
        } else {
            self.queue.entries()
        };
        if sourced {
            self.queue.base_entries = Some(base.clone());
        }
        let mut entries = if shuffle == Shuffle::On {
            permute(base, self.preferences.mode)
        } else {
            base
        };
        if shuffle == Shuffle::On {
            if let Some(i) = pin.and_then(|p| entries.iter().position(|e| e.source_index == p)) {
                let e = entries.remove(i);
                entries.insert(0, e);
            }
        }
        self.queue.install_entries(entries, pin);
        if !sourced {
            self.queue.edited();
        }
    }
    pub fn apply(
        &mut self,
        command: Command,
        position: f64,
        mut permute: impl FnMut(Vec<Entry>, Mode) -> Vec<Entry>,
    ) -> Result<Effect, String> {
        if let Command::SetQueue { queue, .. } | Command::PlayPlaylist { queue, .. } = &command {
            if queue
                .source_indices
                .as_ref()
                .is_some_and(|v| v.len() != queue.tracks.len())
            {
                return Err("Invalid queue membership indexes".into());
            }
            if let Some(entries) = &queue.base_entries {
                let mut ids: Vec<_> = entries.iter().map(|e| e.track.id).collect();
                let mut tracks: Vec<_> = queue.tracks.iter().map(|t| t.id).collect();
                ids.sort_unstable();
                tracks.sort_unstable();
                if ids != tracks {
                    return Err("Invalid base queue".into());
                }
            }
        }
        // Index commands must refer to the queue the user actually saw.
        let expected = match &command {
            Command::Reorder { queue_revision, .. }
            | Command::Remove { queue_revision, .. }
            | Command::Jump { queue_revision, .. } => Some(*queue_revision),
            _ => None,
        };
        if expected.is_some_and(|r| r != self.revision) {
            return Err("Queue changed; please retry the action".into());
        }
        let saves_preferences = command.saves_preferences();
        let previous = self.queue.current().map(|t| t.id);
        let effect = match command {
            Command::Attach { preferences } => {
                if !self.preferences_initialized {
                    if let Some(preferences) = preferences {
                        self.preferences = preferences;
                    }
                    self.preferences_initialized = true;
                    self.revision += 1;
                }
                return Ok(Effect::None);
            }
            Command::SetQueue { mut queue, play } => {
                queue.normalize();
                self.queue = queue;
                if play {
                    self.is_playing = self.queue.current().is_some();
                    self.start("replaced")
                } else if previous != self.queue.current().map(|t| t.id) {
                    self.start("replaced")
                } else {
                    Effect::None
                }
            }
            Command::PlayPlaylist {
                mut queue,
                shuffle,
                toggle_if_current,
                pin_start,
            } => {
                queue.normalize();
                let same_membership = self
                    .queue
                    .source
                    .as_ref()
                    .zip(queue.source.as_ref())
                    .is_some_and(|(a, b)| a.playlist_id == b.playlist_id)
                    && self
                        .queue
                        .source_indices
                        .as_ref()
                        .and_then(|v| v.get(self.queue.cursor as usize))
                        .zip(
                            queue
                                .source_indices
                                .as_ref()
                                .and_then(|v| v.get(queue.cursor as usize)),
                        )
                        .is_some_and(|(a, b)| a == b)
                    && self.queue.current().map(|t| t.id) == queue.current().map(|t| t.id);
                if toggle_if_current && same_membership {
                    let effect = self.playing(!self.is_playing);
                    self.revision += 1;
                    return Ok(effect);
                }
                self.queue = queue;
                if let Some(s) = shuffle {
                    self.preferences.shuffle = s;
                }
                if self.preferences.shuffle == Shuffle::On {
                    self.shuffle(Shuffle::On, pin_start, &mut permute);
                }
                self.is_playing = self.queue.current().is_some();
                self.start("replaced")
            }
            Command::PlayTrack { track } => {
                if previous == Some(track.id) {
                    self.playing(!self.is_playing)
                } else {
                    if let Some(i) = self.queue.tracks.iter().position(|t| t.id == track.id) {
                        self.queue.cursor = i as i64;
                    } else {
                        self.queue = Queue {
                            tracks: vec![track],
                            cursor: 0,
                            ..Default::default()
                        };
                    }
                    self.is_playing = true;
                    self.start("replaced")
                }
            }
            Command::Clear => {
                self.queue = Queue::default();
                self.start("stopped")
            }
            Command::ClearUpNext => {
                if self.queue.current().is_some()
                    && self.queue.cursor + 1 < self.queue.tracks.len() as i64
                {
                    self.queue.tracks.truncate(self.queue.cursor as usize + 1);
                    self.queue.edited();
                }
                Effect::None
            }
            Command::Insert { tracks, next } => {
                if tracks.is_empty() {
                    return Ok(Effect::None);
                }
                if self.queue.current().is_none() {
                    self.queue = Queue {
                        tracks,
                        cursor: 0,
                        ..Default::default()
                    };
                    self.is_playing = true;
                    self.start("replaced")
                } else {
                    let at = if next {
                        self.queue.cursor as usize + 1
                    } else {
                        self.queue.tracks.len()
                    };
                    self.queue.tracks.splice(at..at, tracks);
                    self.queue.edited();
                    Effect::None
                }
            }
            Command::Reorder {
                from_index: from,
                to_index: to,
                ..
            } => {
                let len = self.queue.tracks.len() as i64;
                if from < 0 || to < 0 || from >= len || to >= len || from == to {
                    return Ok(Effect::None);
                }
                let track = self.queue.tracks.remove(from as usize);
                self.queue.tracks.insert(to as usize, track);
                self.queue.cursor = remap(self.queue.cursor, from, to);
                self.queue.edited();
                Effect::None
            }
            Command::Remove { mut indices, .. } => {
                indices.retain(|i| *i >= 0 && *i < self.queue.tracks.len() as i64);
                indices.sort_unstable();
                indices.dedup();
                if indices.is_empty() {
                    return Ok(Effect::None);
                }
                let removed = indices.contains(&self.queue.cursor);
                for i in indices.into_iter().rev() {
                    self.queue.tracks.remove(i as usize);
                    if i < self.queue.cursor {
                        self.queue.cursor -= 1;
                    }
                }
                self.queue.edited();
                if self.queue.tracks.is_empty()
                    || (removed && self.queue.cursor >= self.queue.tracks.len() as i64)
                {
                    self.queue = Queue::default();
                    self.start("stopped")
                } else if removed {
                    self.is_playing = true;
                    self.start("skipped")
                } else {
                    Effect::None
                }
            }
            Command::Jump { index, .. } => {
                let old = self.queue.cursor;
                self.queue.cursor = index;
                self.queue.normalize();
                if old != self.queue.cursor {
                    self.is_playing = self.queue.current().is_some();
                    self.start("replaced")
                } else {
                    self.playing(true)
                }
            }
            Command::Playing { playing } => self.playing(playing),
            Command::Toggle => self.playing(!self.is_playing),
            Command::Next => {
                if self.queue.current().is_none() {
                    return Ok(Effect::None);
                }
                if self.queue.cursor + 1 == self.queue.tracks.len() as i64
                    && self.preferences.repeat == Repeat::None
                {
                    self.playing(false)
                } else {
                    self.queue.cursor = (self.queue.cursor + 1) % self.queue.tracks.len() as i64;
                    self.is_playing = true;
                    self.start("skipped")
                }
            }
            Command::Previous { restart } => {
                if self.queue.current().is_none() {
                    return Ok(Effect::None);
                }
                if restart && position >= 5.0 {
                    Effect::Seek(0.0)
                } else if self.queue.cursor == 0 && self.preferences.repeat == Repeat::None {
                    Effect::None
                } else {
                    self.queue.cursor = (self.queue.cursor + self.queue.tracks.len() as i64 - 1)
                        % self.queue.tracks.len() as i64;
                    self.is_playing = true;
                    self.start("skipped")
                }
            }
            Command::Repeat { repeat } => {
                self.preferences.repeat = repeat;
                Effect::None
            }
            Command::ToggleRepeat => {
                self.preferences.repeat = match self.preferences.repeat {
                    Repeat::None => Repeat::One,
                    Repeat::One => Repeat::All,
                    Repeat::All => Repeat::None,
                };
                Effect::None
            }
            Command::ToggleShuffle => {
                let shuffle = if self.preferences.shuffle == Shuffle::On {
                    Shuffle::Off
                } else {
                    Shuffle::On
                };
                self.shuffle(shuffle, true, &mut permute);
                Effect::None
            }
            Command::Shuffle { shuffle } => {
                self.shuffle(shuffle, true, &mut permute);
                Effect::None
            }
            Command::ShuffleMode { mode } => {
                self.preferences.mode = mode;
                self.shuffle(Shuffle::On, true, &mut permute);
                Effect::None
            }
            Command::Realign {
                playlist_id,
                tracks,
                move_indices,
            } => {
                let Some(source) = self
                    .queue
                    .source
                    .as_mut()
                    .filter(|s| s.playlist_id == playlist_id)
                else {
                    return Ok(Effect::None);
                };
                source.track_ids = tracks.iter().map(|t| t.id).collect();
                if self.preferences.shuffle == Shuffle::On {
                    if let Some([from, to]) = move_indices {
                        if let Some(indices) = self.queue.source_indices.as_mut() {
                            for i in indices {
                                *i = remap(*i as i64, from, to) as usize;
                            }
                        }
                        if let Some(entries) = self.queue.base_entries.as_mut() {
                            for e in entries {
                                e.source_index = remap(e.source_index as i64, from, to) as usize;
                            }
                        }
                    }
                } else {
                    let cursor = if let Some([from, to]) = move_indices {
                        remap(self.queue.cursor, from, to)
                    } else {
                        let occurrence = self
                            .queue
                            .tracks
                            .iter()
                            .take(self.queue.cursor.max(0) as usize)
                            .filter(|t| Some(t.id) == previous)
                            .count();
                        tracks
                            .iter()
                            .enumerate()
                            .filter(|(_, t)| Some(t.id) == previous)
                            .nth(occurrence)
                            .or_else(|| {
                                tracks
                                    .iter()
                                    .enumerate()
                                    .find(|(_, t)| Some(t.id) == previous)
                            })
                            .map(|(i, _)| i as i64)
                            .unwrap_or(0)
                    };
                    self.queue.tracks = tracks;
                    self.queue.cursor = cursor;
                    self.queue.normalize();
                    self.queue.source_indices = Some((0..self.queue.tracks.len()).collect());
                    self.queue.base_entries = Some(self.queue.entries());
                }
                if previous != self.queue.current().map(|t| t.id) {
                    self.start("replaced")
                } else {
                    Effect::None
                }
            }
            Command::Refresh { tracks } => {
                let by_id: std::collections::HashMap<_, _> =
                    tracks.into_iter().map(|t| (t.id, t)).collect();
                let old_cursor = self.queue.cursor.max(0) as usize;
                let mut mapped = None;
                let mut indices = vec![];
                let mut refreshed = vec![];
                for (i, t) in self.queue.tracks.iter().enumerate() {
                    if let Some(t) = by_id.get(&t.id) {
                        if i == old_cursor {
                            mapped = Some(refreshed.len());
                        }
                        refreshed.push(t.clone());
                        if let Some(source) = self.queue.source_indices.as_ref() {
                            indices.push(source[i]);
                        }
                    }
                }
                self.queue.tracks = refreshed;
                self.queue.cursor = mapped.unwrap_or(old_cursor) as i64;
                self.queue.normalize();
                if self.queue.source_indices.is_some() {
                    self.queue.source_indices = Some(indices);
                }
                if let Some(entries) = self.queue.base_entries.as_mut() {
                    entries.retain_mut(|e| {
                        if let Some(t) = by_id.get(&e.track.id) {
                            e.track = t.clone();
                            true
                        } else {
                            false
                        }
                    });
                }
                if let Some(source) = self.queue.source.as_mut() {
                    source.track_ids.retain(|id| by_id.contains_key(id));
                }
                if previous != self.queue.current().map(|t| t.id) {
                    self.start("replaced")
                } else {
                    Effect::None
                }
            }
        };
        if saves_preferences {
            self.preferences_initialized = true;
        }
        self.revision += 1;
        Ok(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(id: i64) -> Track {
        serde_json::from_value(serde_json::json!({"id":id,"tg_user_id":1,"file_id":"file","file_unique_id":"unique","title":"Track","performer":"Artist","duration":120,"source":"saved_music","mime_type":"audio/mpeg","file_size":100,"created_at":"2026-01-01T00:00:00Z"})).unwrap()
    }
    fn queue(ids: &[i64]) -> Queue {
        Queue {
            tracks: ids.iter().map(|id| track(*id)).collect(),
            cursor: 0,
            ..Default::default()
        }
    }
    fn apply(s: &mut Session, c: Command) -> Effect {
        s.apply(c, 0.0, |mut entries, _| {
            entries.reverse();
            entries
        })
        .unwrap()
    }
    fn play(ids: &[i64]) -> Session {
        let mut s = Session::default();
        apply(
            &mut s,
            Command::SetQueue {
                queue: queue(ids),
                play: true,
            },
        );
        s
    }
    fn ids(s: &Session) -> Vec<i64> {
        s.queue.tracks.iter().map(|t| t.id).collect()
    }
    #[test]
    fn advances_without_any_frontend_and_stops_at_end() {
        let mut s = play(&[1, 2, 3]);
        for expected in [2, 3] {
            let attempt = s.attempt;
            assert_eq!(s.complete(attempt), Effect::Load);
            assert_eq!(s.queue.current().unwrap().id, expected);
        }
        let attempt = s.attempt;
        assert_eq!(s.complete(attempt), Effect::None);
        assert!(!s.is_playing);
        assert_eq!(s.queue.cursor, 2);
        assert_eq!(s.complete(attempt), Effect::None);
        assert_eq!(
            apply(&mut s, Command::Playing { playing: true }),
            Effect::Load
        );
        assert!(s.attempt > attempt);
    }
    #[test]
    fn repeat_one_all_and_duplicate_rows_have_unique_attempts() {
        let mut s = play(&[1, 1, 2]);
        let first = s.attempt;
        assert_eq!(s.complete(first), Effect::Load);
        assert_eq!(s.queue.cursor, 1);
        assert_eq!(s.attempt, first + 1);
        assert_eq!(s.complete(first), Effect::None);
        assert_eq!(s.queue.cursor, 1);
        apply(
            &mut s,
            Command::Repeat {
                repeat: Repeat::One,
            },
        );
        let attempt = s.attempt;
        assert_eq!(s.complete(attempt), Effect::Load);
        assert_eq!(s.queue.cursor, 1);
        // Manual next advances even in repeat-one.
        apply(&mut s, Command::Next);
        assert_eq!(s.queue.cursor, 2);
        apply(
            &mut s,
            Command::Repeat {
                repeat: Repeat::All,
            },
        );
        s.complete(s.attempt);
        assert_eq!(s.queue.cursor, 0);
        assert!(s.is_playing);
    }
    #[test]
    fn single_track_wrap_and_duplicate_skip_restart() {
        let mut s = play(&[1]);
        apply(
            &mut s,
            Command::Repeat {
                repeat: Repeat::All,
            },
        );
        let a = s.attempt;
        assert_eq!(apply(&mut s, Command::Next), Effect::Load);
        assert_eq!(s.attempt, a + 1);
        assert_eq!(
            apply(&mut s, Command::Previous { restart: false }),
            Effect::Load
        );
        assert_eq!(s.attempt, a + 2);
        let mut s = play(&[1, 1]);
        let rev = s.revision;
        assert_eq!(
            apply(
                &mut s,
                Command::Jump {
                    index: 1,
                    queue_revision: rev
                }
            ),
            Effect::Load
        );
        assert_eq!(s.attempt, 2);
    }
    #[test]
    fn pause_append_insert_reorder_and_clear_up_next_preserve_attempt() {
        let mut s = play(&[1, 2, 3]);
        apply(&mut s, Command::Playing { playing: false });
        let attempt = s.attempt;
        apply(
            &mut s,
            Command::Insert {
                tracks: vec![track(4)],
                next: true,
            },
        );
        assert_eq!(ids(&s), vec![1, 4, 2, 3]);
        assert!(!s.is_playing);
        apply(
            &mut s,
            Command::Insert {
                tracks: vec![track(5)],
                next: false,
            },
        );
        assert!(!s.is_playing);
        let rev = s.revision;
        apply(
            &mut s,
            Command::Reorder {
                from_index: 0,
                to_index: 2,
                queue_revision: rev,
            },
        );
        assert_eq!(s.queue.current().unwrap().id, 1);
        assert_eq!(s.queue.cursor, 2);
        assert!(!s.is_playing);
        apply(&mut s, Command::ClearUpNext);
        assert_eq!(ids(&s), vec![4, 2, 1]);
        assert_eq!(s.attempt, attempt);
        assert!(!s.is_playing);
    }
    #[test]
    fn idle_insert_autoplays_and_clear_unloads() {
        let mut s = Session::default();
        assert_eq!(
            apply(
                &mut s,
                Command::Insert {
                    tracks: vec![track(1)],
                    next: false
                }
            ),
            Effect::Load
        );
        assert!(s.is_playing);
        assert_eq!(apply(&mut s, Command::Clear), Effect::Unload);
        assert!(!s.is_playing);
        assert_eq!(s.queue.cursor, -1);
    }
    #[test]
    fn removing_current_duplicate_restarts_and_removing_last_current_clears() {
        let mut s = play(&[1, 1, 2]);
        let rev = s.revision;
        let a = s.attempt;
        assert_eq!(
            apply(
                &mut s,
                Command::Remove {
                    indices: vec![0, 0, -1, 100],
                    queue_revision: rev
                }
            ),
            Effect::Load
        );
        assert_eq!(ids(&s), vec![1, 2]);
        assert_eq!(s.attempt, a + 1);
        assert_eq!(s.end_reason, "skipped");
        apply(&mut s, Command::Next);
        let rev = s.revision;
        assert_eq!(
            apply(
                &mut s,
                Command::Remove {
                    indices: vec![1],
                    queue_revision: rev
                }
            ),
            Effect::Unload
        );
        assert!(s.queue.tracks.is_empty());
    }
    #[test]
    fn removing_other_rows_preserves_paused_current_membership() {
        let mut s = play(&[1, 2, 2, 3]);
        apply(&mut s, Command::Next);
        apply(&mut s, Command::Next);
        apply(&mut s, Command::Playing { playing: false });
        let a = s.attempt;
        let rev = s.revision;
        assert_eq!(
            apply(
                &mut s,
                Command::Remove {
                    indices: vec![0, 1],
                    queue_revision: rev
                }
            ),
            Effect::None
        );
        assert_eq!(s.queue.cursor, 0);
        assert_eq!(s.queue.current().unwrap().id, 2);
        assert_eq!(s.attempt, a);
        assert!(!s.is_playing);
    }
    #[test]
    fn stale_row_commands_never_apply_to_a_different_track() {
        let mut s = play(&[1, 2, 3]);
        let rev = s.revision;
        s.complete(s.attempt);
        let before = serde_json::to_value(&s).unwrap();
        for command in [
            Command::Jump {
                index: 0,
                queue_revision: rev,
            },
            Command::Remove {
                indices: vec![1],
                queue_revision: rev,
            },
            Command::Reorder {
                from_index: 0,
                to_index: 2,
                queue_revision: rev,
            },
        ] {
            assert!(s.apply(command, 0.0, |e, _| e).is_err());
            assert_eq!(serde_json::to_value(&s).unwrap(), before);
        }
    }
    #[test]
    fn previous_uses_native_position_and_queue_boundaries() {
        let mut s = play(&[1, 2]);
        apply(&mut s, Command::Next);
        let a = s.attempt;
        assert_eq!(
            s.apply(Command::Previous { restart: true }, 5.0, |e, _| e)
                .unwrap(),
            Effect::Seek(0.0)
        );
        assert_eq!(s.attempt, a);
        assert_eq!(
            s.apply(Command::Previous { restart: true }, 4.9, |e, _| e)
                .unwrap(),
            Effect::Load
        );
        assert_eq!(s.queue.cursor, 0);
        assert_eq!(
            apply(&mut s, Command::Previous { restart: false }),
            Effect::None
        );
    }
    fn sourced() -> Queue {
        // Sorted B,A2,A1; pin the second A membership, not just its track id.
        let tracks = vec![track(2), track(1), track(1)];
        let indices = vec![2, 1, 0];
        Queue {
            source: Some(Source {
                r#type: "playlist".into(),
                playlist_id: 1.into(),
                name: "List".into(),
                track_ids: vec![1, 1, 2],
            }),
            base_entries: Some(
                tracks
                    .iter()
                    .zip(&indices)
                    .map(|(t, i)| Entry {
                        track: t.clone(),
                        source_index: *i,
                    })
                    .collect(),
            ),
            source_indices: Some(indices),
            tracks,
            cursor: 1,
        }
    }
    #[test]
    fn shuffle_restores_sorted_base_and_pins_duplicate_membership_without_restarting() {
        let mut s = Session::default();
        apply(
            &mut s,
            Command::SetQueue {
                queue: sourced(),
                play: true,
            },
        );
        let a = s.attempt;
        apply(
            &mut s,
            Command::Shuffle {
                shuffle: Shuffle::On,
            },
        );
        assert_eq!(s.queue.cursor, 0);
        assert_eq!(s.queue.source_indices.as_ref().unwrap()[0], 1);
        apply(&mut s, Command::ShuffleMode { mode: Mode::Smart });
        assert_eq!(s.queue.source_indices.as_ref().unwrap()[0], 1);
        apply(
            &mut s,
            Command::Shuffle {
                shuffle: Shuffle::Off,
            },
        );
        assert_eq!(ids(&s), vec![2, 1, 1]);
        assert_eq!(s.queue.cursor, 1);
        assert_eq!(s.attempt, a);
    }
    #[test]
    fn edited_shuffle_keeps_current_and_does_not_restore_stale_source() {
        let mut s = play(&[1, 2, 3]);
        apply(&mut s, Command::Next);
        let a = s.attempt;
        apply(
            &mut s,
            Command::Shuffle {
                shuffle: Shuffle::On,
            },
        );
        assert_eq!(s.queue.current().unwrap().id, 2);
        let shuffled = ids(&s);
        apply(
            &mut s,
            Command::Shuffle {
                shuffle: Shuffle::Off,
            },
        );
        assert_eq!(ids(&s), shuffled);
        assert!(s.queue.source.is_none());
        assert_eq!(s.attempt, a);
    }
    #[test]
    fn realign_shuffled_source_remaps_base_memberships() {
        let mut s = Session::default();
        apply(
            &mut s,
            Command::SetQueue {
                queue: sourced(),
                play: true,
            },
        );
        apply(
            &mut s,
            Command::Shuffle {
                shuffle: Shuffle::On,
            },
        );
        let a = s.attempt;
        apply(
            &mut s,
            Command::Realign {
                playlist_id: 1.into(),
                tracks: vec![track(1), track(2), track(1)],
                move_indices: Some([1, 2]),
            },
        );
        assert_eq!(
            s.queue.source_indices.as_ref().unwrap()[s.queue.cursor as usize],
            2
        );
        apply(
            &mut s,
            Command::Shuffle {
                shuffle: Shuffle::Off,
            },
        );
        assert_eq!(
            s.queue.source_indices.as_ref().unwrap()[s.queue.cursor as usize],
            2
        );
        assert_eq!(s.attempt, a);
    }
    #[test]
    fn refresh_preserves_duplicate_membership_and_filters_missing_tracks() {
        let mut s = play(&[1, 1, 2]);
        apply(&mut s, Command::Next);
        let a = s.attempt;
        apply(
            &mut s,
            Command::Refresh {
                tracks: vec![track(1)],
            },
        );
        assert_eq!(s.queue.cursor, 1);
        assert_eq!(s.attempt, a);
        assert_eq!(
            apply(&mut s, Command::Refresh { tracks: vec![] }),
            Effect::Unload
        );
    }
    #[test]
    fn attach_does_not_mutate_live_queue_modes_or_attempt() {
        let mut s = play(&[1, 2]);
        apply(
            &mut s,
            Command::Repeat {
                repeat: Repeat::All,
            },
        );
        let before = serde_json::to_value(&s).unwrap();
        assert_eq!(
            apply(
                &mut s,
                Command::Attach {
                    preferences: Some(Preferences::default())
                }
            ),
            Effect::None
        );
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
    #[test]
    fn invalid_membership_payload_is_rejected_atomically() {
        let mut s = play(&[1]);
        let before = serde_json::to_value(&s).unwrap();
        let mut bad = queue(&[2]);
        bad.source_indices = Some(vec![]);
        assert!(s
            .apply(
                Command::SetQueue {
                    queue: bad,
                    play: true
                },
                0.0,
                |e, _| e
            )
            .is_err());
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
    #[test]
    fn rapid_commands_are_ordered_and_toggle_uses_native_state() {
        let mut s = play(&[1, 2]);
        let a = s.attempt;
        apply(&mut s, Command::Next);
        apply(&mut s, Command::Previous { restart: false });
        assert_eq!(s.attempt, a + 2);
        assert_eq!(s.queue.current().unwrap().id, 1);
        apply(&mut s, Command::Toggle);
        apply(&mut s, Command::Toggle);
        assert!(s.is_playing);
    }
    #[test]
    fn playlist_row_toggle_uses_current_native_membership_after_background_advance() {
        let mut s = Session::default();
        let row = sourced();
        apply(
            &mut s,
            Command::PlayPlaylist {
                queue: row.clone(),
                shuffle: None,
                toggle_if_current: true,
                pin_start: true,
            },
        );
        let first = s.attempt;
        assert_eq!(
            apply(
                &mut s,
                Command::PlayPlaylist {
                    queue: row.clone(),
                    shuffle: None,
                    toggle_if_current: true,
                    pin_start: true,
                }
            ),
            Effect::Pause
        );
        assert_eq!(s.attempt, first);
        apply(&mut s, Command::Playing { playing: true });
        s.complete(s.attempt); // The UI still displays the previously playing duplicate.
        assert_eq!(s.queue.cursor, 2);
        assert_eq!(
            apply(
                &mut s,
                Command::PlayPlaylist {
                    queue: row,
                    shuffle: None,
                    toggle_if_current: true,
                    pin_start: true,
                }
            ),
            Effect::Load
        );
        assert_eq!(s.queue.cursor, 1);
        assert!(s.is_playing);
    }
    #[test]
    fn playlist_shuffle_without_selected_start_does_not_pin_first_row() {
        for shuffle in [None, Some(Shuffle::On)] {
            let mut s = Session::default();
            s.preferences.shuffle = Shuffle::On;
            let mut queue = sourced();
            queue.cursor = 0;
            apply(
                &mut s,
                Command::PlayPlaylist {
                    queue,
                    shuffle,
                    toggle_if_current: false,
                    pin_start: false,
                },
            );
            // Both toolbar shuffle and an inherited shuffle mode accept the
            // permutation, rather than pinning the first base membership (2).
            assert_eq!(s.queue.source_indices.as_ref().unwrap(), &[0, 1, 2]);
            assert_eq!(s.queue.cursor, 0);
        }
    }
    #[test]
    fn playlist_shuffle_pins_an_explicit_duplicate_membership() {
        let mut s = Session::default();
        apply(
            &mut s,
            Command::PlayPlaylist {
                queue: sourced(),
                shuffle: Some(Shuffle::On),
                toggle_if_current: true,
                pin_start: true,
            },
        );
        // Both memberships 0 and 1 have the same track ID; row 1 was selected.
        assert_eq!(s.queue.source_indices.as_ref().unwrap(), &[1, 0, 2]);
        assert_eq!(s.queue.cursor, 0);
        assert_eq!(s.queue.current().unwrap().id, 1);
    }
    #[test]
    fn rapid_shuffle_toggles_use_native_state_and_restore_base_order() {
        let mut s = Session::default();
        apply(
            &mut s,
            Command::SetQueue {
                queue: sourced(),
                play: true,
            },
        );
        apply(&mut s, Command::ToggleShuffle);
        apply(&mut s, Command::ToggleShuffle);
        assert_eq!(s.preferences.shuffle, Shuffle::Off);
        assert_eq!(ids(&s), vec![2, 1, 1]);
        assert_eq!(s.queue.cursor, 1);
    }
    #[test]
    fn preferences_migrate_once_even_if_library_refresh_precedes_attachment() {
        let legacy = Preferences {
            repeat: Repeat::All,
            shuffle: Shuffle::On,
            mode: Mode::Smart,
        };
        let mut s = Session::with_preferences(None);
        apply(&mut s, Command::Refresh { tracks: vec![] });
        let revision = s.revision;
        apply(
            &mut s,
            Command::Attach {
                preferences: Some(legacy),
            },
        );
        assert!(s.revision > revision);
        assert_eq!(s.preferences.repeat, Repeat::All);
        let persisted = serde_json::to_string(&s.preferences).unwrap();
        let mut restored = Session::with_preferences(Some(&persisted));
        apply(
            &mut restored,
            Command::Attach {
                preferences: Some(Preferences::default()),
            },
        );
        assert_eq!(restored.preferences.repeat, Repeat::All);
        assert_eq!(restored.preferences.mode, Mode::Smart);
    }
    #[test]
    fn long_native_session_keeps_membership_and_attempt_invariants() {
        let mut s = play(&[1, 1, 2, 3, 3]);
        apply(
            &mut s,
            Command::Repeat {
                repeat: Repeat::All,
            },
        );
        for i in 0..3000 {
            let old = s.attempt;
            match i % 7 {
                0 => {
                    s.complete(old);
                    assert_eq!(s.complete(old), Effect::None);
                }
                1 => {
                    apply(&mut s, Command::Next);
                }
                2 => {
                    apply(&mut s, Command::Previous { restart: false });
                }
                3 => {
                    apply(
                        &mut s,
                        Command::Shuffle {
                            shuffle: Shuffle::On,
                        },
                    );
                }
                4 => {
                    let rev = s.revision;
                    apply(
                        &mut s,
                        Command::Reorder {
                            from_index: 0,
                            to_index: 4,
                            queue_revision: rev,
                        },
                    );
                }
                5 => {
                    apply(&mut s, Command::Playing { playing: false });
                    assert_eq!(s.complete(s.attempt), Effect::None);
                }
                _ => {
                    apply(&mut s, Command::Playing { playing: true });
                }
            }
            assert!(s.attempt >= old);
            assert!(s.queue.current().is_some());
            let mut tracks = ids(&s);
            tracks.sort_unstable();
            assert_eq!(tracks, vec![1, 1, 2, 3, 3]);
        }
    }
    #[test]
    fn json_contract_contains_ui_queue_and_backend_attempt() {
        let mut s = Session::default();
        let c: Command = serde_json::from_value(
            serde_json::json!({"type":"setQueue","queue":queue(&[1,1]),"play":true}),
        )
        .unwrap();
        apply(&mut s, c);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["attempt"], 1);
        assert_eq!(json["queue"]["tracks"][0]["id"], 1);
        assert!(json["queue"]["tracks"][0].get("mtproto_document").is_none());
        assert_eq!(json["isPlaying"], true);
    }
}
