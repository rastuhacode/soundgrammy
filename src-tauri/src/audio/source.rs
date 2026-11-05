//! A blocking decoder waits on bounded replies; all Telegram work stays async.
use crate::{
    cache,
    state::AppState,
    streaming::{TrackStream, CHUNK_SIZE},
};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};
use symphonia::core::io::MediaSource;
use tauri::{AppHandle, Manager};

#[cfg(test)]
#[path = "source/diagnostics.rs"]
mod diagnostics;
#[path = "source/read_ahead.rs"]
mod read_ahead;

// Cancellation must not use Interrupted: read_exact and demuxers retry it forever.
pub fn wait<T>(rx: &mpsc::Receiver<T>, active: &AtomicBool) -> io::Result<T> {
    loop {
        if !active.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "cancelled",
            ));
        }
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(value) => return Ok(value),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => return Err(io::Error::other("source-unavailable")),
        }
    }
}
struct Session {
    app: AppHandle,
    id: String,
}
impl Drop for Session {
    fn drop(&mut self) {
        let app = self.app.clone();
        let id = self.id.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            state.streaming.close_playback(&id).await;
            let _ = cache::enforce_ttl(&state, &app).await;
        });
    }
}
trait RangeReader: Send + Sync {
    fn read(&self, start: u64, end: u64, active: Arc<AtomicBool>) -> io::Result<Vec<u8>>;
    fn cancel_read_ahead(&self) {}
}
struct StreamReader {
    stream: Arc<TrackStream>,
    downloaded_only: bool,
    coverage: Arc<super::availability::Availability>,
    diagnostic: Arc<Mutex<Option<String>>>,
    read_ahead: Option<read_ahead::ReadAhead>,
}
impl RangeReader for StreamReader {
    fn read(&self, start: u64, end: u64, active: Arc<AtomicBool>) -> io::Result<Vec<u8>> {
        if let Some(ahead) = &self.read_ahead {
            ahead.update(end, active.clone());
        }
        let stream = self.stream.clone();
        let downloaded_only = self.downloaded_only;
        let background = self.read_ahead.is_some();
        let coverage = self.coverage.clone();
        let token = active.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        tauri::async_runtime::spawn(async move {
            // Always settle shared chunk state; never abort the downloader future.
            let result = if downloaded_only {
                stream.read_downloaded_range(start, end, token).await
            } else if background {
                stream.read_background_range(start, end, token).await
            } else {
                stream.read_range(start, end, Some(token)).await
            };
            let result = result.map_err(|error| io::Error::other(error.to_string()));
            if stream.received().await == stream.total() {
                coverage.complete.store(true, Ordering::Release);
            }
            let _ = tx.send(result);
        });
        let result = wait(&rx, &active)?;
        if let Err(error) = &result {
            if error.kind() != io::ErrorKind::ConnectionAborted {
                *self
                    .diagnostic
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                    Some(format!("range {start}..={end}: {error}"));
            }
        }
        result
    }

    fn cancel_read_ahead(&self) {
        if let Some(ahead) = &self.read_ahead {
            ahead.cancel();
        }
    }
}
pub struct Source {
    file: Option<File>,
    stream: Option<Box<dyn RangeReader>>,
    active: Arc<AtomicBool>,
    seek: Option<Arc<super::seek::SeekControl>>,
    cursor: u64,
    total: u64,
    block_start: u64,
    block: Vec<u8>,
    _session: Option<Session>,
    _lease: Option<cache::CacheLease>,
}
pub struct OpenOptions {
    pub active: Arc<AtomicBool>,
    pub observer_active: Arc<AtomicBool>,
    pub seek: Arc<super::seek::SeekControl>,
    pub coverage: Arc<super::availability::Availability>,
    pub diagnostic: Arc<Mutex<Option<String>>>,
}
impl Source {
    pub fn open(
        app: AppHandle,
        track_id: i64,
        generation: u64,
        options: OpenOptions,
    ) -> io::Result<Self> {
        let OpenOptions {
            active,
            observer_active,
            seek,
            coverage,
            diagnostic,
        } = options;
        let id = format!("native-{generation}");
        let session = Session {
            app: app.clone(),
            id: id.clone(),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let token = active.clone();
        let open_diagnostic = diagnostic.clone();
        tauri::async_runtime::spawn(async move {
            let result = async {
                let state = app.state::<AppState>();
                let track = cache::require_track(&state, track_id)?;
                let path = cache::audio_path(&state, &track)?;
                let lease = {
                    let _guard = state.cache_lifecycle.lock().await;
                    state.cache_leases.acquire(&track.file_unique_id)
                };
                // Protection is registered before opening either source, including cache hits.
                if !token.load(Ordering::Acquire) {
                    return Err(crate::error::AppError::msg("cancelled"));
                }
                state.audio.protect(generation, path.clone());
                if path.exists() {
                    let file = File::open(path)?;
                    let total = file.metadata()?.len();
                    let _ = state.db.touch_audio_cache(track_id);
                    return Ok::<_, crate::error::AppError>((Some(file), None, total, lease));
                }
                let stream = state
                    .streaming
                    .open_playback_with_active(app.clone(), track, path, id.clone(), token.clone())
                    .await?;
                if !token.load(Ordering::Acquire) {
                    state.streaming.close_playback(&id).await;
                    return Err(crate::error::AppError::msg("cancelled"));
                }
                let total = stream.total();
                Ok((None, Some(stream), total, lease))
            }
            .await
            .map_err(|error| {
                let message = error.to_string();
                *open_diagnostic
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                    Some(format!("opening playback source: {message}"));
                io::Error::other(message)
            });
            let _ = tx.send(result);
        });
        let (file, stream, total, lease) = wait(&rx, &active)??;
        if file.is_some() {
            coverage.complete.store(true, Ordering::Release);
        }
        if let Some(stream) = &stream {
            spawn_prefetch(stream.clone(), active.clone(), coverage.clone())?;
            spawn_observer(
                stream.clone(),
                observer_active,
                coverage.clone(),
                diagnostic.clone(),
            )?;
        }
        Ok(Self {
            file,
            stream: stream.map(|stream| {
                Box::new(StreamReader {
                    stream,
                    downloaded_only: false,
                    coverage,
                    diagnostic,
                    read_ahead: None,
                }) as Box<dyn RangeReader>
            }),
            active,
            seek: Some(seek),
            cursor: 0,
            total,
            block_start: 0,
            block: Vec::new(),
            _session: Some(session),
            _lease: Some(lease),
        })
    }
}

pub fn spawn_observer(
    stream: Arc<TrackStream>,
    active: Arc<AtomicBool>,
    coverage: Arc<super::availability::Availability>,
    diagnostic: Arc<Mutex<Option<String>>>,
) -> io::Result<()> {
    let observer = Source {
        file: None,
        stream: Some(Box::new(StreamReader {
            stream: stream.clone(),
            downloaded_only: true,
            coverage: coverage.clone(),
            diagnostic,
            read_ahead: None,
        })),
        active: active.clone(),
        seek: Some(coverage.scan_seek.clone()),
        cursor: 0,
        total: stream.total(),
        block_start: 0,
        block: Vec::new(),
        _session: None,
        _lease: None,
    };
    spawn_scanner(Box::new(observer), active, coverage).map(|_| ())
}

fn spawn_prefetch(
    stream: Arc<TrackStream>,
    active: Arc<AtomicBool>,
    coverage: Arc<super::availability::Availability>,
) -> io::Result<()> {
    std::thread::Builder::new()
        .name("native-audio-prefetch".into())
        .spawn(move || {
            run_prefetch(&active, &coverage, || {
                let ahead = read_ahead::ReadAhead::new(stream.clone(), active.clone());
                Box::new(Source {
                    file: None,
                    stream: Some(Box::new(StreamReader {
                        stream: stream.clone(),
                        downloaded_only: false,
                        coverage: coverage.clone(),
                        // Speculation must not overwrite demanded-read diagnostics.
                        diagnostic: Arc::new(Mutex::new(None)),
                        read_ahead: Some(ahead),
                    })),
                    active: active.clone(),
                    seek: Some(coverage.prefetch_seek.clone()),
                    cursor: 0,
                    total: stream.total(),
                    block_start: 0,
                    block: Vec::new(),
                    _session: None,
                    _lease: None,
                })
            });
        })
        .map(|_| ())
}

fn run_prefetch(
    active: &AtomicBool,
    coverage: &super::availability::Availability,
    source: impl Fn() -> Box<dyn MediaSource>,
) {
    while active.load(Ordering::Acquire) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::availability::prefetch(source(), active, coverage);
        }));
        // A background range may fail while the foreground request recovers.
        // Re-probe verified data rather than leaving recovery waiting forever,
        // or retrying a demuxer whose partial packet parse was interrupted.
        for _ in 0..10 {
            if !active.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        coverage.prefetch_seek.cancel_read();
        coverage
            .prefetch_seek
            .interruptible
            .store(false, Ordering::Release);
        coverage.prefetch_seek.request(coverage.position());
    }
}

pub(super) fn spawn_scanner(
    source: Box<dyn MediaSource>,
    active: Arc<AtomicBool>,
    coverage: Arc<super::availability::Availability>,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("native-audio-availability".into())
        .spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                super::availability::scan(source, &active, &coverage)
            }));
        })
        .map_err(|_| io::Error::other("availability observer unavailable"))
}
impl Read for Source {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if !self.active.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "cancelled",
            ));
        }
        if self.seek.as_ref().is_some_and(|seek| {
            seek.interruptible.load(Ordering::Acquire)
                && seek.revision.load(Ordering::Acquire)
                    != seek.decoder_revision.load(Ordering::Acquire)
        }) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "seek superseded",
            ));
        }
        if out.is_empty() || self.cursor >= self.total {
            return Ok(0);
        }
        if let Some(file) = self.file.as_mut() {
            let n = file.read(out)?;
            self.cursor += n as u64;
            return Ok(n);
        }
        if self.cursor < self.block_start
            || self.cursor >= self.block_start + self.block.len() as u64
        {
            let start = self.cursor / CHUNK_SIZE * CHUNK_SIZE;
            let end = (start + CHUNK_SIZE - 1).min(self.total - 1);
            let read_active = self
                .seek
                .as_ref()
                .map_or_else(|| self.active.clone(), |seek| seek.begin_read());
            if !self.active.load(Ordering::Acquire)
                || self.seek.as_ref().is_some_and(|seek| {
                    seek.interruptible.load(Ordering::Acquire)
                        && seek.revision.load(Ordering::Acquire)
                            != seek.decoder_revision.load(Ordering::Acquire)
                })
            {
                read_active.store(false, Ordering::Release);
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "cancelled",
                ));
            }
            let result = self
                .stream
                .as_ref()
                .ok_or_else(|| io::Error::other("source-unavailable"))?
                .read(start, end, read_active);
            self.block = result?;
            if self.block.len() != (end - start + 1) as usize {
                return Err(io::Error::other("short source read"));
            }
            self.block_start = start;
        }
        let offset = (self.cursor - self.block_start) as usize;
        let n = out.len().min(self.block.len() - offset);
        out[..n].copy_from_slice(&self.block[offset..offset + n]);
        self.cursor += n as u64;
        Ok(n)
    }
}
impl Seek for Source {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(p) => self.total as i128 + p as i128,
            SeekFrom::Current(p) => self.cursor as i128 + p as i128,
        };
        if position < 0 || position > self.total as i128 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid seek"));
        }
        if self.cursor != position as u64 {
            if let Some(stream) = &self.stream {
                stream.cancel_read_ahead();
            }
        }
        self.cursor = position as u64;
        if let Some(file) = self.file.as_mut() {
            file.seek(SeekFrom::Start(self.cursor))?;
        }
        Ok(self.cursor)
    }
}
impl MediaSource for Source {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct FakeRanges {
        requests: Arc<Mutex<Vec<u64>>>,
    }
    impl RangeReader for FakeRanges {
        fn read(&self, start: u64, end: u64, _: Arc<AtomicBool>) -> io::Result<Vec<u8>> {
            self.requests.lock().unwrap().push(start);
            Ok((start..=end).map(|i| (i % 251) as u8).collect())
        }
    }
    #[test]
    fn range_cache_and_seek_only_read_verified_requested_blocks() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mut source = Source {
            file: None,
            stream: Some(Box::new(FakeRanges {
                requests: requests.clone(),
            })),
            active: Arc::new(AtomicBool::new(true)),
            seek: None,
            cursor: 0,
            total: CHUNK_SIZE * 20,
            block_start: 0,
            block: Vec::new(),
            _session: None,
            _lease: None,
        };
        let mut bytes = [0; 16];
        source.read_exact(&mut bytes).unwrap();
        source.read_exact(&mut bytes).unwrap();
        assert_eq!(requests.lock().unwrap().len(), 1);
        source.seek(SeekFrom::Start(CHUNK_SIZE * 12 + 7)).unwrap();
        source.read_exact(&mut bytes).unwrap();
        assert_eq!(*requests.lock().unwrap(), vec![0, CHUNK_SIZE * 12]);
        assert_eq!(bytes[0], ((CHUNK_SIZE * 12 + 7) % 251) as u8);
        assert!(source.seek(SeekFrom::End(1)).is_err());
        let seek = Arc::new(super::super::seek::SeekControl::default());
        seek.interruptible.store(true, Ordering::Release);
        source.seek = Some(seek.clone());
        seek.request(75.0);
        // read_exact must return cancellation rather than retry forever.
        assert_eq!(
            source.read_exact(&mut bytes).unwrap_err().kind(),
            io::ErrorKind::ConnectionAborted
        );
        source.active.store(false, Ordering::Release);
        assert_eq!(
            source.read_exact(&mut bytes).unwrap_err().kind(),
            io::ErrorKind::ConnectionAborted
        );
    }
    #[test]
    fn cancellation_wakes_blocked_decoder_without_network_completion() {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
        let active = Arc::new(AtomicBool::new(true));
        let token = active.clone();
        let worker = std::thread::spawn(move || wait(&rx, &token));
        active.store(false, Ordering::Release);
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::ConnectionAborted
        );
        drop(tx);
    }
}

#[cfg(test)]
mod streaming_decode_tests {
    use super::*;
    use ringbuf::{traits::*, HeapRb};
    use std::{sync::Mutex, time::Instant};
    struct FileRanges {
        bytes: Vec<u8>,
        requests: Arc<Mutex<Vec<u64>>>,
    }
    impl RangeReader for FileRanges {
        fn read(&self, start: u64, end: u64, _: Arc<AtomicBool>) -> io::Result<Vec<u8>> {
            self.requests.lock().unwrap().push(start);
            Ok(self.bytes[start as usize..=end as usize].to_vec())
        }
    }
    #[test]
    fn prefetch_reprobes_after_a_failed_background_range() {
        use std::sync::atomic::AtomicUsize;
        struct FailsOnce {
            bytes: Arc<Vec<u8>>,
            fail: Arc<AtomicBool>,
        }
        impl RangeReader for FailsOnce {
            fn read(&self, start: u64, end: u64, _: Arc<AtomicBool>) -> io::Result<Vec<u8>> {
                if start >= CHUNK_SIZE && self.fail.swap(false, Ordering::AcqRel) {
                    return Err(io::Error::other("injected background request failure"));
                }
                Ok(self.bytes[start as usize..=end as usize].to_vec())
            }
        }
        let bytes = Arc::new(include_bytes!("../../tests/fixtures/audio/long-tone.mp3").to_vec());
        let active = Arc::new(AtomicBool::new(true));
        let coverage = Arc::new(super::super::availability::Availability::default());
        let attempts = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicBool::new(true));
        let token = active.clone();
        let observer = coverage.clone();
        let opens = attempts.clone();
        let worker = std::thread::spawn(move || {
            run_prefetch(&token, &observer, || {
                opens.fetch_add(1, Ordering::AcqRel);
                Box::new(Source {
                    file: None,
                    stream: Some(Box::new(FailsOnce {
                        bytes: bytes.clone(),
                        fail: failed.clone(),
                    })),
                    active: token.clone(),
                    seek: Some(observer.prefetch_seek.clone()),
                    cursor: 0,
                    total: bytes.len() as u64,
                    block_start: 0,
                    block: Vec::new(),
                    _session: None,
                    _lease: None,
                })
            });
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while !coverage.has_headroom(0.1, 31.9, 120.0) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let recovered = coverage.has_headroom(0.1, 31.9, 120.0);
        active.store(false, Ordering::Release);
        coverage.prefetch_seek.cancel_read();
        worker.join().unwrap();
        assert!(recovered, "background prefetch did not recover");
        assert!(attempts.load(Ordering::Acquire) >= 2);
    }

    #[test]
    fn prefetch_maps_thirty_two_seconds_without_pcm_and_moves_with_playback() {
        let mp3 = include_bytes!("../../tests/fixtures/audio/long-tone.mp3").to_vec();
        let second = include_bytes!("../../tests/fixtures/audio/tone.wav");
        let mut wav = second[..44].to_vec();
        for _ in 0..120 {
            wav.extend_from_slice(&second[44..]);
        }
        let size = wav.len() as u32;
        wav[4..8].copy_from_slice(&(size - 8).to_le_bytes());
        wav[40..44].copy_from_slice(&(size - 44).to_le_bytes());
        for bytes in [mp3, wav] {
            let total = bytes.len() as u64;
            let requests = Arc::new(Mutex::new(Vec::new()));
            let active = Arc::new(AtomicBool::new(true));
            let coverage = Arc::new(super::super::availability::Availability::default());
            let source = Source {
                file: None,
                stream: Some(Box::new(FileRanges {
                    bytes,
                    requests: requests.clone(),
                })),
                active: active.clone(),
                seek: Some(coverage.prefetch_seek.clone()),
                cursor: 0,
                total,
                block_start: 0,
                block: Vec::new(),
                _session: None,
                _lease: None,
            };
            let token = active.clone();
            let observer = coverage.clone();
            let worker = std::thread::spawn(move || {
                super::super::availability::prefetch(Box::new(source), &token, &observer);
            });
            let wait_for = |start: f64, end: f64| {
                let deadline = Instant::now() + Duration::from_secs(3);
                while Instant::now() < deadline {
                    if coverage.has_headroom(start, end - start, 120.0) {
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                false
            };
            let initial = wait_for(0.1, 32.0);
            std::thread::sleep(Duration::from_millis(60));
            let initial_end = coverage.ranges(120.0).last().map(|r| r.end).unwrap_or(0.0);
            coverage.set_position(20.0);
            let rolling = wait_for(20.0, 52.0);
            coverage.request_seek(95.0);
            let sought = wait_for(95.1, 119.9);
            active.store(false, Ordering::Release);
            coverage.prefetch_seek.cancel_read();
            worker.join().unwrap();
            assert!(
                initial && rolling && sought,
                "missing initial, rolling, or sought coverage"
            );
            assert!(
                initial_end < 32.1,
                "prefetch must stop at its time target, got {initial_end}"
            );
            assert!(
                !requests
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|&offset| { offset > total * 3 / 5 && offset < total * 7 / 10 }),
                "seek must not download the middle gap"
            );
        }
    }

    #[test]
    fn mp3_seek_downloads_target_without_filling_the_gap() {
        let bytes = include_bytes!("../../tests/fixtures/audio/long-tone.mp3").to_vec();
        let total = bytes.len() as u64;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let active = Arc::new(AtomicBool::new(true));
        let source = Source {
            file: None,
            stream: Some(Box::new(FileRanges {
                bytes,
                requests: requests.clone(),
            })),
            active: active.clone(),
            seek: None,
            cursor: 0,
            total,
            block_start: 0,
            block: Vec::new(),
            _session: None,
            _lease: None,
        };
        let (producer, consumer) = HeapRb::<f32>::new(4096).split();
        let (sender, _receiver) = mpsc::sync_channel(8);
        let token = active.clone();
        let worker = std::thread::spawn(move || {
            super::super::decode::decode(
                Box::new(source),
                90.0,
                (48000, 2),
                producer,
                token,
                Arc::new(super::super::output::Clock::default()),
                sender,
            )
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while consumer.occupied_len() == 0 && !worker.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let buffered = consumer.occupied_len();
        active.store(false, Ordering::Release);
        assert!(worker.join().unwrap().is_ok());
        assert!(buffered > 0, "seek must produce playable PCM");
        let requested = requests.lock().unwrap();
        assert!(requested.iter().any(|offset| *offset > total * 2 / 3));
        assert!(
            !requested
                .iter()
                .any(|offset| *offset > total / 4 && *offset < total / 2),
            "seek must leave the intervening chunks undownloaded: {requested:?}"
        );
    }
    #[test]
    fn decode_starts_before_complete_download_and_seek_prioritizes_target() {
        for origin in [0.0, 5.0] {
            let one_second = include_bytes!("../../tests/fixtures/audio/tone.wav");
            let mut bytes = one_second[..44].to_vec();
            for _ in 0..20 {
                bytes.extend_from_slice(&one_second[44..]);
            }
            let size = bytes.len() as u32;
            bytes[4..8].copy_from_slice(&(size - 8).to_le_bytes());
            bytes[40..44].copy_from_slice(&(size - 44).to_le_bytes());
            let requests = Arc::new(Mutex::new(Vec::new()));
            let active = Arc::new(AtomicBool::new(true));
            let source = Source {
                file: None,
                stream: Some(Box::new(FileRanges {
                    bytes,
                    requests: requests.clone(),
                })),
                active: active.clone(),
                seek: None,
                cursor: 0,
                total: size as u64,
                block_start: 0,
                block: Vec::new(),
                _session: None,
                _lease: None,
            };
            let (producer, consumer) = HeapRb::<f32>::new(4096).split();
            let (sender, _receiver) = mpsc::sync_channel(8);
            let token = active.clone();
            let worker = std::thread::spawn(move || {
                super::super::decode::decode(
                    Box::new(source),
                    origin,
                    (48000, 2),
                    producer,
                    token,
                    Arc::new(super::super::output::Clock::default()),
                    sender,
                )
            });
            let deadline = Instant::now() + Duration::from_secs(3);
            while consumer.occupied_len() == 0 && !worker.is_finished() && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            let buffered = consumer.occupied_len();
            active.store(false, Ordering::Release);
            assert!(worker.join().unwrap().is_ok());
            assert!(buffered > 0);
            let requested = requests.lock().unwrap();
            assert!(
                requested.len() < (size as u64 / CHUNK_SIZE) as usize,
                "must not fetch entire file"
            );
            if origin > 0.0 {
                assert!(requested.iter().any(|offset| *offset >= CHUNK_SIZE * 6));
            }
        }
    }
}
