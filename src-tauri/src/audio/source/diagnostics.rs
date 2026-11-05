//! Opt-in, headless measurement of decoder headroom at an uncached range miss.
//! No Telegram account, network, audio device, or wall-clock playback is needed.
use super::*;
use ringbuf::{traits::*, HeapRb};
use std::time::Instant;

struct GatedRanges {
    bytes: Vec<u8>,
    requested: mpsc::Sender<u64>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl RangeReader for GatedRanges {
    fn read(&self, start: u64, end: u64, active: Arc<AtomicBool>) -> io::Result<Vec<u8>> {
        self.requested.send(start).unwrap();
        // Hold the first non-leading chunk as if its RPC were still in flight.
        if start == CHUNK_SIZE {
            wait(&self.release.lock().unwrap(), &active)?;
        }
        Ok(self.bytes[start as usize..=end as usize].to_vec())
    }
}

#[test]
#[ignore = "diagnostic: run with --ignored --nocapture to measure range-miss headroom"]
fn uncached_range_miss_headroom() {
    let mp3 = include_bytes!("../../../tests/fixtures/audio/long-tone.mp3").to_vec();
    let wav_second = include_bytes!("../../../tests/fixtures/audio/tone.wav");
    let mut wav = wav_second[..44].to_vec();
    for _ in 0..20 {
        wav.extend_from_slice(&wav_second[44..]);
    }
    let size = wav.len() as u32;
    wav[4..8].copy_from_slice(&(size - 8).to_le_bytes());
    wav[40..44].copy_from_slice(&(size - 44).to_le_bytes());

    for (format, bytes) in [("128 kbps MP3", mp3), ("1411 kbps WAV", wav)] {
        let active = Arc::new(AtomicBool::new(true));
        let (requested, requests) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let source = Source {
            total: bytes.len() as u64,
            file: None,
            stream: Some(Box::new(GatedRanges {
                bytes,
                requested,
                release: Mutex::new(gate),
            })),
            active: active.clone(),
            seek: None,
            cursor: 0,
            block_start: 0,
            block: Vec::new(),
            _session: None,
            _lease: None,
        };
        let (producer, mut consumer) = HeapRb::<f32>::new(48000 * 2 * 2).split();
        let clock = Arc::new(super::super::output::Clock::default());
        let (sender, _messages) = mpsc::sync_channel(32);
        let token = active.clone();
        let decode_clock = clock.clone();
        let worker = std::thread::spawn(move || {
            super::super::decode::decode(
                Box::new(source),
                0.0,
                (48000, 2),
                producer,
                token,
                decode_clock,
                sender,
            )
        });
        // Advance a virtual device in small steps. This measures media seconds,
        // not network timing, throughput, or audible device behavior.
        let mut consumed_frames = 0u64;
        let mut scratch = [0.0; 960];
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut offsets = Vec::new();
        let mut held = false;
        while Instant::now() < deadline && !worker.is_finished() {
            while let Ok(offset) = requests.try_recv() {
                offsets.push(offset);
                held |= offset == CHUNK_SIZE;
            }
            if held {
                break;
            }
            consumed_frames += consumer.pop_slice(&mut scratch) as u64 / 2;
            std::thread::sleep(Duration::from_millis(1));
        }
        let queued_frames = consumer.occupied_len() as u64 / 2;
        let produced = clock.produced.load(Ordering::Acquire);
        // Drain everything already decoded while the missing range stays held.
        while consumer.pop_slice(&mut scratch) > 0 {}
        std::thread::sleep(Duration::from_millis(20));
        let refilled = consumer.occupied_len();
        let _ = release.send(());
        active.store(false, Ordering::Release);
        let result = worker.join().unwrap();
        println!(
            "{format}: offsets={offsets:?}, produced={:.3}s, consumed={:.3}s, \
             PCM remaining at range miss={:.3}s, refill while blocked={refilled} samples",
            produced as f64 / 48000.0,
            consumed_frames as f64 / 48000.0,
            queued_frames as f64 / 48000.0,
        );
        assert!(held, "decoder did not reach the gated range: {result:?}");
        assert!(queued_frames <= 48000 * 2);
        assert_eq!(
            refilled, 0,
            "decoder replenished PCM without the missing range"
        );
    }
}
