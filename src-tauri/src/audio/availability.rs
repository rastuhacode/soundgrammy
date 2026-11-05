//! Persistent media-time coverage from demuxed, locally present packets.
//! This is independent of decoded PCM and never estimates time from byte ratios.
use super::Range;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
#[derive(Default)]
pub struct Availability {
    pub complete: AtomicBool,
    pub scan_seek: Arc<super::seek::SeekControl>,
    pub prefetch_seek: Arc<super::seek::SeekControl>,
    position: AtomicU64,
    media_end: AtomicU64,
    ranges: Mutex<Vec<Range>>,
}
impl Availability {
    pub fn set_position(&self, seconds: f64) {
        self.position.store(seconds.to_bits(), Ordering::Release);
    }

    pub fn position(&self) -> f64 {
        f64::from_bits(self.position.load(Ordering::Acquire))
    }

    pub fn request_seek(&self, seconds: f64) {
        self.set_position(seconds);
        self.scan_seek.request(seconds);
        self.prefetch_seek.request(seconds);
    }

    pub fn has_headroom(&self, position: f64, seconds: f64, duration: f64) -> bool {
        let media_end = f64::from_bits(self.media_end.load(Ordering::Acquire));
        let duration = if media_end > 0.0 { media_end } else { duration };
        let target = if duration > 0.0 {
            (position + seconds).min(duration)
        } else {
            position + seconds
        };
        self.complete.load(Ordering::Acquire)
            || self
                .ranges(duration)
                .iter()
                .any(|range| range.start <= position + 0.002 && range.end + 0.002 >= target)
    }

    pub fn record(&self, start: f64, end: f64) {
        if !start.is_finite() || !end.is_finite() || end <= start {
            return;
        }
        let mut ranges = self.ranges.lock().unwrap_or_else(|e| e.into_inner());
        ranges.push(Range {
            start: start.max(0.0),
            end,
        });
        ranges.sort_by(|a, b| a.start.total_cmp(&b.start));
        let mut i = 1;
        while i < ranges.len() {
            if ranges[i].start <= ranges[i - 1].end + 0.002 {
                ranges[i - 1].end = ranges[i - 1].end.max(ranges[i].end);
                ranges.remove(i);
            } else {
                i += 1;
            }
        }
        // Bound disjoint coverage retained for pathological media.
        ranges.truncate(1024);
    }
    pub fn ranges(&self, duration: f64) -> Vec<Range> {
        if self.complete.load(Ordering::Acquire) && duration > 0.0 {
            return vec![Range {
                start: 0.0,
                end: duration,
            }];
        }
        self.ranges
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(|range| {
                let end = if duration > 0.0 {
                    range.end.min(duration)
                } else {
                    range.end
                };
                (end > range.start).then_some(Range {
                    start: range.start,
                    end,
                })
            })
            .collect()
    }
}

/// Demux downloaded bytes ahead of playback, without decoding PCM or fetching data.
/// Packet timestamps work for VBR and indexed containers without byte/time guesses.
pub fn scan(
    source: Box<dyn symphonia::core::io::MediaSource>,
    active: &AtomicBool,
    coverage: &Availability,
) {
    scan_inner(source, active, coverage, &coverage.scan_seek, false);
}

/// Separate playback-owned demux cursor: fetch ahead by actual packet time.
/// The passive display scanner above retains its storage-only lifetime.
pub fn prefetch(
    source: Box<dyn symphonia::core::io::MediaSource>,
    active: &AtomicBool,
    coverage: &Availability,
) {
    scan_inner(source, active, coverage, &coverage.prefetch_seek, true);
}

pub const REMOTE_LOAD_AHEAD_SECONDS: f64 = 32.0;
pub const RECOVERY_SECONDS: f64 = 3.0;

fn scan_inner(
    source: Box<dyn symphonia::core::io::MediaSource>,
    active: &AtomicBool,
    coverage: &Availability,
    seek: &super::seek::SeekControl,
    prefetch: bool,
) {
    use symphonia::core::{
        common::Limit,
        formats::{probe::Hint, FormatOptions, SeekMode, SeekTo, TrackType},
        io::MediaSourceStream,
        meta::MetadataOptions,
        units::Time,
    };
    let Ok(mut format) = symphonia::default::get_probe().probe(
        &Hint::new(),
        MediaSourceStream::new(source, Default::default()),
        FormatOptions::default(),
        MetadataOptions::default()
            .limit_tag_bytes(Limit::Maximum(64 * 1024))
            .limit_visual_bytes(Limit::Maximum(0)),
    ) else {
        return;
    };
    let Some(track) = format.default_track(TrackType::Audio) else {
        return;
    };
    let indexed_end = if format.format_info().short_name == "isomp4"
        && track.num_frames.is_some_and(|frames| frames > 0)
    {
        track
            .duration
            .map(|duration| track.start_ts.saturating_add(duration))
    } else {
        None
    };
    let id = track.id;
    let Some(tb) = track.time_base else {
        return;
    };
    seek.interruptible.store(true, Ordering::Release);
    let mut revision = 0;
    let mut eof = false;
    let mut received_till = 0.0;
    while active.load(Ordering::Acquire) {
        if let Some((next, target)) = seek.take() {
            eof = false;
            received_till = target;
            revision = next;
            seek.decoder_revision.store(next, Ordering::Release);
            let Some(time) = Time::try_new(
                target.trunc() as i64,
                (target.fract() * 1_000_000_000.0) as u32,
            ) else {
                continue;
            };
            if format
                .seek(
                    SeekMode::Coarse,
                    SeekTo::Time {
                        time,
                        track_id: Some(id),
                    },
                )
                .is_err()
            {
                if seek.revision.load(Ordering::Acquire) != revision {
                    continue;
                }
                break;
            }
        }
        let position = coverage.position();
        if eof || (prefetch && received_till >= position + REMOTE_LOAD_AHEAD_SECONDS) {
            std::thread::sleep(std::time::Duration::from_millis(25));
            continue;
        }
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Err(_) if seek.revision.load(Ordering::Acquire) != revision => continue,
            Ok(None) => {
                eof = true;
                coverage
                    .media_end
                    .store(received_till.to_bits(), Ordering::Release);
                continue;
            }
            Err(_) => break,
        };
        if packet.track_id != id {
            continue;
        }
        let Some(start) = tb.calc_time(packet.pts) else {
            continue;
        };
        let Some(end) = tb.calc_time(packet.pts.saturating_add(packet.dur)) else {
            continue;
        };
        received_till = end.as_secs_f64();
        coverage.record(start.as_secs_f64(), received_till);
        // Preserve the last MP4 atom so a later seek can reuse this demuxer.
        eof = indexed_end.is_some_and(|end| packet.pts.saturating_add(packet.dur) >= end);
        if eof {
            coverage
                .media_end
                .store(received_till.to_bits(), Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_requires_three_contiguous_seconds_or_the_remaining_tail() {
        let coverage = Availability::default();
        coverage.record(0.0, 11.0);
        coverage.record(12.0, 20.0);
        assert!(!coverage.has_headroom(10.0, RECOVERY_SECONDS, 60.0));
        coverage.record(11.0, 12.0);
        assert!(coverage.has_headroom(10.0, RECOVERY_SECONDS, 60.0));
        assert!(coverage.has_headroom(19.0, RECOVERY_SECONDS, 20.0));
        assert!(!coverage.has_headroom(30.0, RECOVERY_SECONDS, 60.0));
        coverage.complete.store(true, Ordering::Release);
        assert!(coverage.has_headroom(30.0, RECOVERY_SECONDS, 60.0));
    }

    #[test]
    fn observed_media_end_allows_recovery_with_unknown_or_rounded_duration() {
        let coverage = Availability::default();
        coverage.record(90.0, 100.0);
        coverage
            .media_end
            .store(100.0f64.to_bits(), Ordering::Release);
        assert!(coverage.has_headroom(99.0, RECOVERY_SECONDS, 0.0));
        assert!(coverage.has_headroom(99.0, RECOVERY_SECONDS, 101.0));
        assert!(!coverage.has_headroom(50.0, RECOVERY_SECONDS, 101.0));
    }

    #[test]
    fn playback_seek_cancels_both_observers_and_updates_load_ahead_origin() {
        let coverage = Availability::default();
        coverage
            .scan_seek
            .interruptible
            .store(true, Ordering::Release);
        coverage
            .prefetch_seek
            .interruptible
            .store(true, Ordering::Release);
        let passive = coverage.scan_seek.begin_read();
        let prefetch = coverage.prefetch_seek.begin_read();
        coverage.request_seek(90.0);
        assert!(!passive.load(Ordering::Acquire));
        assert!(!prefetch.load(Ordering::Acquire));
        assert_eq!(coverage.prefetch_seek.take(), Some((1, 90.0)));
        assert_eq!(
            f64::from_bits(coverage.position.load(Ordering::Acquire)),
            90.0
        );
    }

    #[test]
    fn complete_files_and_downloaded_islands_outlive_the_pcm_queue() {
        let coverage = Availability::default();
        coverage.record(0.0, 30.0);
        coverage.record(30.0, 55.0);
        coverage.record(90.0, 100.0);
        let ranges = coverage.ranges(120.0);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].end, 55.0);
        coverage.complete.store(true, Ordering::Release);
        assert_eq!(coverage.ranges(120.0)[0].end, 120.0);
    }
}

#[cfg(test)]
mod scanner_tests {
    use super::*;
    #[test]
    fn scanner_maps_new_islands_after_tail_eof_including_mp4() {
        for bytes in [
            include_bytes!("../../tests/fixtures/audio/long-tone.mp3").as_slice(),
            include_bytes!("../../tests/fixtures/audio/tone.m4a").as_slice(),
        ] {
            let duration = if bytes.len() > 100_000 { 120.0 } else { 1.0 };
            let coverage = Arc::new(Availability::default());
            coverage.scan_seek.request(duration * 0.75);
            let active = Arc::new(AtomicBool::new(true));
            let worker_coverage = coverage.clone();
            let worker_active = active.clone();
            let bytes = bytes.to_vec();
            let worker = std::thread::spawn(move || {
                scan(
                    Box::new(std::io::Cursor::new(bytes)),
                    &worker_active,
                    &worker_coverage,
                )
            });
            let wait_for = |start: f64| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                while std::time::Instant::now() < deadline {
                    let ranges = coverage.ranges(duration);
                    if ranges
                        .iter()
                        .any(|r| r.start <= start && r.end >= duration - 0.05)
                    {
                        return true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                false
            };
            let tail = wait_for(duration * 0.76);
            // Allow next_packet to encounter EOF before requesting another island.
            std::thread::sleep(std::time::Duration::from_millis(50));
            let alive_at_eof = !worker.is_finished();
            coverage.scan_seek.request(duration * 0.25);
            let middle = wait_for(duration * 0.26);
            active.store(false, Ordering::Release);
            worker.join().unwrap();
            assert!(tail, "tail coverage missing");
            assert!(alive_at_eof, "observer must remain available after EOF");
            assert!(
                middle,
                "observer must map new downloaded packets without PCM decoding"
            );
        }
    }
    #[test]
    fn maps_downloaded_media_beyond_pcm_capacity_without_decoding() {
        let fixture = include_bytes!("../../tests/fixtures/audio/tone.wav");
        let mut bytes = fixture[..44].to_vec();
        for _ in 0..12 {
            bytes.extend_from_slice(&fixture[44..]);
        }
        let size = bytes.len() as u32;
        bytes[4..8].copy_from_slice(&(size - 8).to_le_bytes());
        bytes[40..44].copy_from_slice(&(size - 44).to_le_bytes());
        let coverage = Arc::new(Availability::default());
        let active = Arc::new(AtomicBool::new(true));
        let worker_coverage = coverage.clone();
        let worker_active = active.clone();
        let worker = std::thread::spawn(move || {
            scan(
                Box::new(std::io::Cursor::new(bytes)),
                &worker_active,
                &worker_coverage,
            )
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while coverage.ranges(12.0).last().is_none_or(|r| r.end < 11.99)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        active.store(false, Ordering::Release);
        worker.join().unwrap();
        assert!(!coverage.complete.load(Ordering::Acquire));
        let ranges = coverage.ranges(12.0);
        assert_eq!(ranges.len(), 1);
        assert!(ranges[0].start < 0.01);
        assert!(ranges[0].end > 11.99);
    }
}
