use super::convert::Converter;
use ringbuf::{traits::*, HeapProd};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::SyncSender,
    Arc,
};
use std::time::Duration;
use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    common::Limit,
    errors::Error,
    formats::{probe::Hint, FormatOptions, SeekMode, SeekTo, TrackType},
    io::{MediaSource, MediaSourceStream},
    meta::MetadataOptions,
    units::Time,
};

pub enum Message {
    Duration(f64),
    Eof(u64),
    Seeked(u64),
    Failed(&'static str),
}
pub fn classify(error: &Error) -> &'static str {
    match error {
        Error::Unsupported(_) => "unsupported-format",
        Error::IoError(error) if error.kind() == std::io::ErrorKind::Interrupted => "interrupted",
        Error::IoError(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            "decode-failed"
        }
        Error::IoError(_) => "source-unavailable",
        _ => "decode-failed",
    }
}
#[cfg(test)]
pub fn decode(
    source: Box<dyn MediaSource>,
    origin: f64,
    output_spec: (u32, usize),
    producer: HeapProd<f32>,
    active: Arc<AtomicBool>,
    clock: Arc<super::output::Clock>,
    sender: SyncSender<Message>,
) -> Result<(), &'static str> {
    decode_inner(
        source,
        origin,
        output_spec,
        producer,
        active,
        clock,
        sender,
        None,
    )
}

pub struct Live {
    pub seek: Arc<super::seek::SeekControl>,
    pub availability: Arc<super::availability::Availability>,
}
#[allow(clippy::too_many_arguments)]
pub fn decode_inner(
    source: Box<dyn MediaSource>,
    mut origin: f64,
    output_spec: (u32, usize),
    mut producer: HeapProd<f32>,
    active: Arc<AtomicBool>,
    clock: Arc<super::output::Clock>,
    sender: SyncSender<Message>,
    live: Option<Live>,
) -> Result<(), &'static str> {
    let (rate, channels) = output_spec;
    let metadata = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(64 * 1024))
        .limit_visual_bytes(Limit::Maximum(0));
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            MediaSourceStream::new(source, Default::default()),
            FormatOptions::default(),
            metadata,
        )
        .map_err(|e| classify(&e))?;
    let indexed_mp4 = format.format_info().short_name == "isomp4";
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("unsupported-format")?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("unsupported-format")?
        .clone();
    // Preserve MP4's last atom: Symphonia 0.6 cannot seek after its atom iterator
    // has been exhausted. Its declared sample-table end is sufficient for EOF.
    let indexed_end = if indexed_mp4 && track.num_frames.is_some_and(|frames| frames > 0) {
        track
            .duration
            .map(|duration| track.start_ts.saturating_add(duration))
    } else {
        None
    };
    let track_id = track.id;
    let time_base = track.time_base;
    let input_rate = params.sample_rate.ok_or("unsupported-format")?;
    let input_channels = params
        .channels
        .as_ref()
        .ok_or("unsupported-format")?
        .count();
    if !(1..=2).contains(&input_channels) {
        return Err("unsupported-format");
    }
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| classify(&e))?;
    if let Some(frames) = track.num_frames {
        let _ = sender.send(Message::Duration(frames as f64 / input_rate as f64));
    }
    if origin > 0.0 {
        format
            .seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time::try_new(
                        origin.trunc() as i64,
                        (origin.fract() * 1_000_000_000.0) as u32,
                    )
                    .ok_or("decode-failed")?,
                    track_id: Some(track_id),
                },
            )
            .map_err(|e| classify(&e))?;
        decoder.reset();
    }
    let mut converter = Converter::new(input_rate, rate, channels)?;
    let mut samples = Vec::<f32>::new();
    let mut failures = 0;
    let mut decoded_frames = 0;
    let mut push = |pcm: Vec<f32>, epoch: u64| -> bool {
        let mut offset = 0;
        while offset < pcm.len() {
            if !active.load(Ordering::Acquire)
                || live
                    .as_ref()
                    .is_some_and(|l| l.seek.revision.load(Ordering::Acquire) != epoch)
            {
                return false;
            }
            let available = producer.vacant_len() / channels * channels;
            let written =
                producer.push_slice(&pcm[offset..offset + available.min(pcm.len() - offset)]);
            offset += written;
            clock
                .produced
                .fetch_add((written / channels) as u64, Ordering::Release);
            if written == 0 {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        true
    };
    let mut epoch = 0;
    let mut eof = false;
    let mut reached_indexed_end = false;
    if let Some(live) = &live {
        live.seek.interruptible.store(true, Ordering::Release);
    }
    while active.load(Ordering::Acquire) {
        if let Some((revision, target)) = live.as_ref().and_then(|l| l.seek.take()) {
            epoch = revision;
            if let Some(live) = &live {
                live.seek.decoder_revision.store(epoch, Ordering::Release);
            }
            clock.playing.store(false, Ordering::Release);
            clock.reset_request.store(epoch, Ordering::Release);
            while clock.reset_ack.load(Ordering::Acquire) != epoch {
                if !active.load(Ordering::Acquire) {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let started = std::time::Instant::now();
            let result = format.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time::try_new(
                        target.trunc() as i64,
                        (target.fract() * 1_000_000_000.0) as u32,
                    )
                    .ok_or("decode-failed")?,
                    track_id: Some(track_id),
                },
            );
            if live
                .as_ref()
                .is_some_and(|l| l.seek.revision.load(Ordering::Acquire) != epoch)
            {
                continue;
            }
            result.map_err(|e| classify(&e))?;
            decoder.reset();
            converter = Converter::new(input_rate, rate, channels)?;
            origin = target;
            decoded_frames = 0;
            failures = 0;
            eof = false;
            reached_indexed_end = false;
            tracing::debug!(
                seek_revision = epoch,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "native demux seek completed"
            );
            let _ = sender.send(Message::Seeked(epoch));
        }
        if eof {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        let packet = match if reached_indexed_end {
            Ok(None)
        } else {
            format.next_packet()
        } {
            Ok(Some(p)) => p,
            Ok(None) => {
                if decoded_frames == 0 && origin == 0.0 {
                    return Err("decode-failed");
                }
                if !push(converter.push(&[], input_channels, true)?, epoch) {
                    continue;
                }
                let _ = sender.send(Message::Eof(epoch));
                if live.is_none() {
                    return Ok(());
                }
                eof = true;
                continue;
            }
            Err(Error::ResetRequired) if failures < 16 => {
                failures += 1;
                decoder.reset();
                continue;
            }
            Err(_)
                if live
                    .as_ref()
                    .is_some_and(|l| l.seek.revision.load(Ordering::Acquire) != epoch) =>
            {
                continue
            }
            Err(e) => return Err(classify(&e)),
        };
        if packet.track_id != track_id {
            continue;
        }
        reached_indexed_end =
            indexed_end.is_some_and(|end| packet.pts.saturating_add(packet.dur) >= end);
        if packet.data.len() > 8 * 1024 * 1024 {
            return Err("decode-failed");
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(Error::DecodeError(_)) if failures < 16 => {
                failures += 1;
                continue;
            }
            Err(Error::ResetRequired) if failures < 16 => {
                failures += 1;
                decoder.reset();
                continue;
            }
            Err(e) => return Err(classify(&e)),
        };
        failures = 0;
        if buffer.samples_interleaved() > 2 * 192_000
            || buffer.spec().rate() != input_rate
            || buffer.spec().channels().count() != input_channels
        {
            return Err("unsupported-format");
        }
        samples.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut samples);
        let start = time_base
            .and_then(|tb| tb.calc_time(packet.pts))
            .map(|t| t.as_secs_f64())
            .unwrap_or(decoded_frames as f64 / input_rate as f64);
        decoded_frames += buffer.frames();
        let skip = ((origin - start).max(0.0) * input_rate as f64).round() as usize;
        let skip = skip.min(buffer.frames()) * input_channels;
        if let Some(live) = &live {
            live.availability
                .record(start, start + buffer.frames() as f64 / input_rate as f64);
        }
        push(
            converter.push(&samples[skip..], input_channels, false)?,
            epoch,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::HeapRb;
    use std::{fs::File, path::PathBuf, sync::mpsc};

    fn fixture(name: &str, origin: f64) -> Result<Vec<f32>, &'static str> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/audio")
            .join(name);
        let (producer, mut consumer) = HeapRb::<f32>::new(48000 * 2 * 3).split();
        let (sender, receiver) = mpsc::sync_channel(8);
        decode(
            Box::new(File::open(path).unwrap()),
            origin,
            (48000, 2),
            producer,
            Arc::new(AtomicBool::new(true)),
            Arc::new(super::super::output::Clock::default()),
            sender,
        )?;
        assert!(receiver
            .try_iter()
            .any(|message| matches!(message, Message::Eof(_))));
        let mut samples = vec![0.0; consumer.occupied_len()];
        consumer.pop_slice(&mut samples);
        Ok(samples)
    }
    #[test]
    fn codec_matrix_decodes_and_seeks_with_finite_pcm() {
        for name in [
            "tone.wav",
            "tone.mp3",
            "tone.m4a",
            "tone-alac.m4a",
            "tone.ogg",
            "tone.flac",
        ] {
            let full = fixture(name, 0.0).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                full.len() > 80_000 && full.len() < 110_000,
                "{name}: {} samples",
                full.len()
            );
            assert!(full.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!(full.iter().any(|s| s.abs() > 0.05));
            let seek = fixture(name, 0.5).unwrap_or_else(|e| panic!("{name} seek: {e}"));
            assert!(
                seek.len() > 35_000 && seek.len() < 60_000,
                "{name} seek: {} samples",
                seek.len()
            );
        }
    }
    #[test]
    fn opus_is_explicitly_unsupported_in_both_observed_containers() {
        // .opus is supported by Telegram as an audio file, but not SoundGrammy
        // It makes it not fully represent the whole possible Tg library
        // Although .opus is so rare I wouldn't consider fixing it right now
        // TODO: fix if anyone reports
        for name in ["tone.opus", "tone.webm"] {
            assert_eq!(fixture(name, 0.0).unwrap_err(), "unsupported-format");
        }
    }
    #[test]
    fn corrupt_input_fails_without_eof() {
        let (producer, _) = HeapRb::<f32>::new(1024).split();
        let (sender, receiver) = mpsc::sync_channel(8);
        let result = decode(
            Box::new(std::io::Cursor::new(vec![0u8; 4096])),
            0.0,
            (48000, 2),
            producer,
            Arc::new(AtomicBool::new(true)),
            Arc::new(super::super::output::Clock::default()),
            sender,
        );
        assert!(result.is_err());
        assert!(!receiver.try_iter().any(|m| matches!(m, Message::Eof(_))));
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use ringbuf::HeapRb;
    use std::{fs::File, sync::mpsc, time::Instant};

    #[test]
    fn live_worker_seeks_forward_backward_and_after_eof_without_reopening_source() {
        for name in ["tone.wav", "tone.mp3", "tone.m4a", "tone.ogg", "tone.flac"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/audio")
                .join(name);
            let source = Box::new(File::open(path).unwrap());
            let (producer, mut consumer) = HeapRb::<f32>::new(48000 * 2 * 2).split();
            let active = Arc::new(AtomicBool::new(true));
            let clock = Arc::new(super::super::output::Clock::default());
            let seek = Arc::new(super::super::seek::SeekControl::default());
            let (sender, receiver) = mpsc::sync_channel(8);
            let token = active.clone();
            let worker_clock = clock.clone();
            let worker_seek = seek.clone();
            let worker = std::thread::spawn(move || {
                decode_inner(
                    source,
                    0.0,
                    (48000, 2),
                    producer,
                    token,
                    worker_clock,
                    sender,
                    Some(Live {
                        seek: worker_seek,
                        availability: Arc::new(super::super::availability::Availability::default()),
                    }),
                )
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                for target in [None, Some(0.65), Some(0.2)] {
                    let revision = target.map_or(0, |target| {
                        seek.request(0.1); // An obsolete scrub must never land.
                        seek.request(target)
                    });
                    let deadline = Instant::now() + Duration::from_secs(3);
                    let mut eof = false;
                    while Instant::now() < deadline && !eof {
                        super::super::output::reset_consumer(&mut consumer, &clock);
                        while let Ok(message) = receiver.try_recv() {
                            if let Message::Eof(epoch) = message {
                                eof |= epoch == revision;
                            }
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    assert!(
                        eof,
                        "{name}: seek {target:?} did not reach EOF, worker finished: {}",
                        worker.is_finished()
                    );
                    let frames = consumer.occupied_len() / 2;
                    let expected = ((1.0 - target.unwrap_or(0.0)) * 48000.0) as usize;
                    assert!(
                        frames.abs_diff(expected) < 5000,
                        "{name}: stale or missing PCM after seek: {frames} / {expected}"
                    );
                    // Leave old PCM in the queue: the next seek must discard it.
                }
                assert!(
                    !worker.is_finished(),
                    "source worker must remain reusable after EOF"
                );
            }));
            active.store(false, Ordering::Release);
            seek.cancel_read();
            let decoded = worker.join().unwrap();
            assert!(decoded.is_ok(), "{name}: {decoded:?}");
            result.unwrap();
        }
    }
}
