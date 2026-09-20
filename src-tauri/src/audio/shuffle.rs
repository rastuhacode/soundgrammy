//! The existing six shuffle policies, evaluated from native library statistics.
use super::session::{Entry, Mode};
use crate::db::{Db, TrackListenStats, SETTING_LISTEN_STATS_ENABLED};
use std::collections::HashMap;

pub fn shuffle(entries: Vec<Entry>, mode: Mode, db: &Db) -> Vec<Entry> {
    let stats = db
        .all_listen_stats()
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s.track_id, s))
        .collect();
    let enabled = db
        .get_setting(SETTING_LISTEN_STATS_ENABLED)
        .ok()
        .flatten()
        .as_deref()
        != Some("false");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;
    permute(
        entries,
        mode,
        &stats,
        enabled,
        now,
        &mut rand::random::<f64>,
    )
}
fn randomize(entries: &mut [Entry], random: &mut impl FnMut() -> f64) {
    for i in (1..entries.len()).rev() {
        let j = (random() * (i + 1) as f64) as usize;
        entries.swap(i, j.min(i));
    }
}
fn performer(e: &Entry) -> Option<String> {
    e.track
        .performer
        .as_ref()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
}
fn weight(e: &Entry, mode: Mode, stats: &HashMap<i64, TrackListenStats>, now: f64) -> f64 {
    if mode == Mode::Fresh {
        let parsed = chrono::DateTime::parse_from_rfc3339(&e.track.created_at)
            .map(|d| d.timestamp_millis())
            .ok()
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(&e.track.created_at, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .map(|d| d.and_utc().timestamp_millis())
            });
        return parsed
            .map(|t| 1.0 + 4.0 * (-((now - t as f64).max(0.0) / 86_400_000.0) / 30.0).exp())
            .unwrap_or(1.0);
    }
    let Some(s) = stats.get(&e.track.id).filter(|s| s.starts > 0) else {
        return if mode == Mode::Rediscover { 8.0 } else { 3.5 };
    };
    let days = s
        .last_played_at_ms
        .map(|t| (now - t as f64).max(0.0) / 86_400_000.0)
        .unwrap_or(365.0);
    if mode == Mode::Rediscover {
        1.0 + (days / 30.0).min(7.0) + 1.0 / (s.starts as f64).sqrt()
    } else {
        (1.0 + s.likeness.ln_1p() + (days / 90.0).min(2.0)
            - 2.0 * s.early_skips as f64 / s.starts.max(1) as f64)
            .max(0.15)
    }
}
pub(super) fn permute(
    mut entries: Vec<Entry>,
    mode: Mode,
    stats: &HashMap<i64, TrackListenStats>,
    enabled: bool,
    now: f64,
    random: &mut impl FnMut() -> f64,
) -> Vec<Entry> {
    if mode == Mode::Fresh || (enabled && matches!(mode, Mode::Smart | Mode::Rediscover)) {
        let mut scored: Vec<_> = entries
            .into_iter()
            .map(|e| {
                let key =
                    -random().max(f64::MIN_POSITIVE).ln() / weight(&e, mode, stats, now).max(0.01);
                (e, key)
            })
            .collect();
        scored.sort_by(|a, b| a.1.total_cmp(&b.1));
        return scored.into_iter().map(|(e, _)| e).collect();
    }
    randomize(&mut entries, random);
    if mode == Mode::Variety {
        let mut out = vec![];
        let mut recent = vec![];
        while !entries.is_empty() {
            let eligible: Vec<_> = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| performer(e).is_none_or(|p| !recent.contains(&p)))
                .map(|(i, _)| i)
                .collect();
            let i = if eligible.is_empty() {
                (random() * entries.len() as f64) as usize
            } else {
                eligible[(random() * eligible.len() as f64) as usize]
            };
            let e = entries.remove(i);
            if let Some(p) = performer(&e) {
                recent.push(p);
                if recent.len() > 3 {
                    recent.remove(0);
                }
            }
            out.push(e);
        }
        return out;
    }
    if mode == Mode::Duration {
        let mut durations: Vec<_> = entries
            .iter()
            .filter_map(|e| e.track.duration.filter(|d| *d > 0))
            .collect();
        durations.sort_unstable();
        if durations.len() < 2 {
            return entries;
        }
        let median = durations[durations.len() / 2];
        let short: Vec<_> = entries
            .iter()
            .filter(|e| e.track.duration.is_some_and(|d| d > 0 && d <= median))
            .cloned()
            .collect();
        let long: Vec<_> = entries
            .iter()
            .filter(|e| e.track.duration.is_some_and(|d| d > median))
            .cloned()
            .collect();
        if short.is_empty() || long.is_empty() {
            return entries;
        }
        let mut take_short = short.len() >= long.len();
        let mut short = short.into_iter();
        let mut long = long.into_iter();
        let mut out = vec![];
        loop {
            let next = if take_short {
                short.next().or_else(|| long.next())
            } else {
                long.next().or_else(|| short.next())
            };
            let Some(e) = next else { break };
            out.push(e);
            take_short = !take_short;
        }
        for e in entries
            .into_iter()
            .filter(|e| e.track.duration.is_none_or(|d| d <= 0))
        {
            let i = (random() * (out.len() + 1) as f64) as usize;
            out.insert(i, e);
        }
        return out;
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entries() -> Vec<Entry> {
        (0..8).map(|i| Entry {source_index:i,track:serde_json::from_value(serde_json::json!({"id":i/2+1,"tg_user_id":1,"file_id":"","file_unique_id":"","title":"","performer":format!("Artist {}",i),"duration":([60,70,80,90,300,310,320,330][i]),"source":"saved_music","mime_type":null,"file_size":null,"created_at":"2026-01-01T00:00:00Z"})).unwrap()}).collect()
    }
    fn seeded(seed: u32) -> impl FnMut() -> f64 {
        let mut state = seed;
        move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state as f64 / 4_294_967_296.0
        }
    }
    fn indices(entries: &[Entry]) -> Vec<usize> {
        entries.iter().map(|e| e.source_index).collect()
    }
    #[test]
    fn every_mode_preserves_duplicate_memberships_for_many_seeds() {
        for mode in [
            Mode::Random,
            Mode::Variety,
            Mode::Rediscover,
            Mode::Smart,
            Mode::Fresh,
            Mode::Duration,
        ] {
            for seed in 0..100 {
                let result = permute(
                    entries(),
                    mode,
                    &HashMap::new(),
                    true,
                    0.0,
                    &mut seeded(seed),
                );
                let mut ids = indices(&result);
                ids.sort_unstable();
                assert_eq!(ids, (0..8).collect::<Vec<_>>());
            }
        }
    }
    #[test]
    fn disabled_statistics_use_random_order() {
        let expected = permute(
            entries(),
            Mode::Random,
            &HashMap::new(),
            false,
            0.0,
            &mut seeded(9),
        );
        for mode in [Mode::Smart, Mode::Rediscover] {
            assert_eq!(
                indices(&permute(
                    entries(),
                    mode,
                    &HashMap::new(),
                    false,
                    0.0,
                    &mut seeded(9)
                )),
                indices(&expected)
            );
        }
    }
    #[test]
    fn variety_spaces_performers_and_duration_alternates_groups() {
        let mut tracks = entries();
        tracks.truncate(6);
        tracks[0].track.performer = Some("Same".into());
        tracks[5].track.performer = Some("Same".into());
        let mixed = permute(
            tracks,
            Mode::Variety,
            &HashMap::new(),
            true,
            0.0,
            &mut seeded(7),
        );
        let positions: Vec<_> = mixed
            .iter()
            .enumerate()
            .filter(|(_, e)| e.track.performer.as_deref() == Some("Same"))
            .map(|(i, _)| i)
            .collect();
        assert!(positions[1] - positions[0] > 3);
        let mixed = permute(
            entries(),
            Mode::Duration,
            &HashMap::new(),
            true,
            0.0,
            &mut seeded(3),
        );
        assert_eq!(
            mixed
                .iter()
                .take(6)
                .map(|e| e.track.duration.unwrap() <= 300)
                .collect::<Vec<_>>(),
            [true, false, true, false, true, false]
        );
    }
    #[test]
    fn weights_favor_discovery_favorites_and_recent_additions() {
        let mut tracks = entries();
        tracks.truncate(2);
        tracks[0].track.id = 1;
        tracks[1].track.id = 2;
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-10T12:00:00Z")
            .unwrap()
            .timestamp_millis() as f64;
        let stats = TrackListenStats {
            track_id: 2,
            starts: 20,
            qualified_plays: 1,
            completes: 0,
            early_skips: 0,
            total_listened_ms: 0,
            first_played_at_ms: None,
            last_played_at_ms: Some(now as i64),
            likeness: 1.0,
        };
        let mut by_id = HashMap::from([(2, stats.clone())]);
        let wins = |mode, map: &HashMap<i64, TrackListenStats>, tracks: &Vec<Entry>| {
            (1..=100)
                .filter(|seed| {
                    permute(tracks.clone(), mode, map, true, now, &mut seeded(*seed))[0]
                        .track
                        .id
                        == 1
                })
                .count()
        };
        assert!(wins(Mode::Rediscover, &by_id, &tracks) > 75);
        by_id.insert(
            1,
            TrackListenStats {
                track_id: 1,
                starts: 10,
                likeness: 5.0,
                ..stats.clone()
            },
        );
        by_id.insert(
            2,
            TrackListenStats {
                starts: 10,
                early_skips: 10,
                likeness: 0.0,
                ..stats
            },
        );
        assert!(wins(Mode::Smart, &by_id, &tracks) > 80);
        tracks[0].track.created_at = "2026-08-10T00:00:00Z".into();
        tracks[1].track.created_at = "2020-01-01T00:00:00Z".into();
        assert!(wins(Mode::Fresh, &by_id, &tracks) > 70);
    }
}
