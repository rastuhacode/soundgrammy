//! Persistent media-time coverage from demuxed, locally present packets.
//! This is independent of decoded PCM and never estimates time from byte ratios.
use super::Range;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
#[derive(Default)]
pub struct Availability {
    pub complete: AtomicBool,
    pub scan_seek: Arc<super::seek::SeekControl>,
    ranges: Mutex<Vec<Range>>,
}
impl Availability {
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
    coverage
        .scan_seek
        .interruptible
        .store(true, Ordering::Release);
    let mut revision = 0;
    let mut eof = false;
    while active.load(Ordering::Acquire) {
        if let Some((next, target)) = coverage.scan_seek.take() {
            eof = false;
            revision = next;
            coverage
                .scan_seek
                .decoder_revision
                .store(next, Ordering::Release);
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
                if coverage.scan_seek.revision.load(Ordering::Acquire) != revision {
                    continue;
                }
                break;
            }
        }
        if eof {
            std::thread::sleep(std::time::Duration::from_millis(25));
            continue;
        }
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Err(_) if coverage.scan_seek.revision.load(Ordering::Acquire) != revision => continue,
            Ok(None) => {
                eof = true;
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
        coverage.record(start.as_secs_f64(), end.as_secs_f64());
        // Preserve the last MP4 atom so a later seek can reuse this demuxer.
        eof = indexed_end.is_some_and(|end| packet.pts.saturating_add(packet.dur) >= end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
