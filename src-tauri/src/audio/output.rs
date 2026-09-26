//! Only this module touches CPAL. The owning control thread drops the stream.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{traits::*, HeapCons, HeapProd, HeapRb};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};

#[derive(Default)]
pub struct Clock {
    pub playing: AtomicBool,
    pub reset_request: AtomicU64,
    pub reset_ack: AtomicU64,
    pub consumed: AtomicU64,
    /// Lifetime PCM frames; never reset by seek.
    pub rendered: AtomicU64,
    pub presented: AtomicU64,
    pub produced: AtomicU64,
    pub underruns: AtomicU64,
    pub failed: AtomicBool,
    pub gain: AtomicU32,
}
pub struct Output {
    pub stream: cpal::Stream,
    pub rate: u32,
    pub channels: usize,
    pub clock: Arc<Clock>,
}
pub fn sanitize(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}
fn render<T: cpal::SizedSample + cpal::FromSample<f32>>(
    data: &mut [T],
    consumer: &mut HeapCons<f32>,
    channels: usize,
    clock: &Clock,
) {
    let gain = f32::from_bits(clock.gain.load(Ordering::Relaxed));
    let mut consumed = 0;
    let playing = clock.playing.load(Ordering::Acquire);
    let mut starved = false;
    for frame in data.chunks_mut(channels) {
        let available = playing && !starved && consumer.occupied_len() >= channels;
        starved |= !available;
        for sample in frame {
            *sample = T::from_sample(if available {
                sanitize(consumer.try_pop().unwrap_or(0.0) * gain)
            } else {
                0.0
            });
        }
        if available {
            consumed += 1;
        }
    }
    clock.consumed.fetch_add(consumed, Ordering::Relaxed);
    clock.rendered.fetch_add(consumed, Ordering::Release);
    if playing && consumed == 0 {
        clock.underruns.fetch_add(1, Ordering::Relaxed);
    }
}
#[derive(Clone, Copy, Default)]
struct Presentation {
    start_ns: u128,
    end_ns: u128,
    first_frame: u64,
    frames: u64,
}
struct PresentationClock {
    slots: [Presentation; 256],
    head: usize,
    len: usize,
}
impl Default for PresentationClock {
    fn default() -> Self {
        Self {
            slots: [Presentation::default(); 256],
            head: 0,
            len: 0,
        }
    }
}
impl PresentationClock {
    fn advance(&mut self, now: u128, clock: &Clock) {
        while self.len > 0 {
            let front = self.slots[self.head];
            if now < front.start_ns {
                break;
            }
            let elapsed = now
                .saturating_sub(front.start_ns)
                .min(front.end_ns - front.start_ns);
            let frames =
                (elapsed * front.frames as u128 / (front.end_ns - front.start_ns).max(1)) as u64;
            clock
                .presented
                .fetch_max(front.first_frame + frames, Ordering::Release);
            if now < front.end_ns {
                break;
            }
            self.head = (self.head + 1) % self.slots.len();
            self.len -= 1;
        }
    }
    fn push(&mut self, sample: Presentation, clock: &Clock) {
        if self.len == self.slots.len() {
            clock.failed.store(true, Ordering::Release);
            return;
        }
        self.slots[(self.head + self.len) % self.slots.len()] = sample;
        self.len += 1;
    }
}
/// The decode producer must be quiescent until reset_ack matches reset_request.
/// The callback owns the consumer and performs this bounded, lock-free flush.
pub(super) fn reset_consumer(consumer: &mut HeapCons<f32>, clock: &Clock) -> bool {
    let reset = clock.reset_request.load(Ordering::Acquire);
    if reset == clock.reset_ack.load(Ordering::Relaxed) {
        return false;
    }
    consumer.skip(consumer.occupied_len());
    clock.consumed.store(0, Ordering::Relaxed);
    clock.presented.store(0, Ordering::Relaxed);
    clock.produced.store(0, Ordering::Relaxed);
    clock.reset_ack.store(reset, Ordering::Release);
    true
}

fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut consumer: HeapCons<f32>,
    clock: Arc<Clock>,
) -> Result<cpal::Stream, &'static str> {
    let channels = config.channels as usize;
    let error_clock = clock.clone();
    let rate = config.sample_rate;
    let mut presentation = PresentationClock::default();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info| {
                if reset_consumer(&mut consumer, &clock) {
                    presentation = PresentationClock::default();
                    data.fill(T::from_sample(0.0));
                    return;
                }
                let timestamp = info.timestamp();
                presentation.advance(timestamp.callback.as_nanos(), &clock);
                let first_frame = clock.consumed.load(Ordering::Relaxed);
                render(data, &mut consumer, channels, &clock);
                let frames = clock.consumed.load(Ordering::Relaxed) - first_frame;
                if frames > 0 {
                    let start_ns = timestamp.playback.as_nanos();
                    presentation.push(
                        Presentation {
                            start_ns,
                            end_ns: start_ns + frames as u128 * 1_000_000_000 / rate as u128,
                            first_frame,
                            frames,
                        },
                        &clock,
                    );
                }
            },
            move |_| {
                error_clock.failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|error| {
            tracing::warn!(?config, %error, "could not build native audio output");
            "output-unavailable"
        })
}

fn supported_format(format: cpal::SampleFormat) -> bool {
    matches!(
        format,
        cpal::SampleFormat::F32
            | cpal::SampleFormat::F64
            | cpal::SampleFormat::I8
            | cpal::SampleFormat::I16
            | cpal::SampleFormat::I32
            | cpal::SampleFormat::I64
            | cpal::SampleFormat::U8
            | cpal::SampleFormat::U16
            | cpal::SampleFormat::U32
            | cpal::SampleFormat::U64
    )
}

fn usable_config(config: &cpal::SupportedStreamConfig) -> bool {
    (1..=8).contains(&config.channels())
        && (8_000..=192_000).contains(&config.sample_rate())
        && supported_format(config.sample_format())
}

fn fallback_configs(
    default: Option<&cpal::SupportedStreamConfig>,
    ranges: impl Iterator<Item = cpal::SupportedStreamConfigRange>,
) -> Vec<cpal::SupportedStreamConfig> {
    let preferred_rate = default.map_or(48_000, |c| c.sample_rate());
    let preferred_channels = default.map_or(2, |c| c.channels());
    let mut configs: Vec<_> = ranges
        .filter_map(|range| {
            let min = range.min_sample_rate().max(8_000);
            let max = range.max_sample_rate().min(192_000);
            if min > max {
                return None;
            }
            let config = range.with_sample_rate(preferred_rate.clamp(min, max));
            (usable_config(&config) && Some(&config) != default).then_some(config)
        })
        .collect();
    // Preserve the active route's rate before preferring stereo/float. Picking
    // the last equally ranked range can otherwise force a headset out of its
    // microphone profile (or request a rate that is unavailable while recording).
    configs.sort_by_key(|c| {
        (
            c.sample_rate().abs_diff(preferred_rate),
            c.channels() != preferred_channels,
            c.channels() != 2,
            c.sample_format() != cpal::SampleFormat::F32,
        )
    });
    configs.dedup();
    configs
}
impl Output {
    pub fn open(volume: f64) -> Result<(Self, HeapProd<f32>), &'static str> {
        #[cfg(target_os = "ios")]
        super::ios::activate()?;
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("output-unavailable")?;
        let default = device
            .default_output_config()
            .map_err(|error| {
                tracing::warn!(%error, "could not query default native audio format");
            })
            .ok();
        // Open the current device format before enumerating alternatives. A
        // Bluetooth microphone can make this mono at a voice sample rate, and
        // enumeration may fail even though this format is usable.
        if let Some(config) = default.as_ref().filter(|c| usable_config(c)) {
            if let Ok(output) = Self::open_config(&device, *config, volume) {
                return Ok(output);
            }
        }
        let ranges = device.supported_output_configs().map_err(|error| {
            tracing::warn!(%error, "could not enumerate native audio formats");
            "output-unavailable"
        })?;
        for config in fallback_configs(default.as_ref(), ranges) {
            if let Ok(output) = Self::open_config(&device, config, volume) {
                return Ok(output);
            }
        }
        Err("output-unavailable")
    }

    fn open_config(
        device: &cpal::Device,
        config: cpal::SupportedStreamConfig,
        volume: f64,
    ) -> Result<(Self, HeapProd<f32>), &'static str> {
        let rate = config.sample_rate();
        let channels = config.channels() as usize;
        // Two seconds maximum decode-ahead; startup watermark is 100ms.
        let (producer, consumer) = HeapRb::<f32>::new(rate as usize * channels * 2).split();
        let clock = Arc::new(Clock::default());
        clock.gain.store(
            (volume.clamp(0.0, 100.0) as f32 / 100.0).to_bits(),
            Ordering::Relaxed,
        );
        macro_rules! stream {
            ($t:ty) => {
                build::<$t>(device, config.config(), consumer, clock.clone())?
            };
        }
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => stream!(f32),
            cpal::SampleFormat::F64 => stream!(f64),
            cpal::SampleFormat::I8 => stream!(i8),
            cpal::SampleFormat::I16 => stream!(i16),
            cpal::SampleFormat::I32 => stream!(i32),
            cpal::SampleFormat::I64 => stream!(i64),
            cpal::SampleFormat::U8 => stream!(u8),
            cpal::SampleFormat::U16 => stream!(u16),
            cpal::SampleFormat::U32 => stream!(u32),
            cpal::SampleFormat::U64 => stream!(u64),
            _ => return Err("output-unavailable"),
        };
        stream.play().map_err(|error| {
            tracing::warn!(?config, %error, "could not start native audio output");
            "output-unavailable"
        })?;
        Ok((
            Self {
                stream,
                rate,
                channels,
                clock,
            },
            producer,
        ))
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.clock.playing.store(false, Ordering::Release);
        let _ = self.stream.pause();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(channels: u16, min: u32, max: u32) -> cpal::SupportedStreamConfigRange {
        cpal::SupportedStreamConfigRange::new(
            channels,
            min,
            max,
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::F32,
        )
    }

    #[test]
    fn headset_fallback_preserves_active_rate_before_stereo_preference() {
        let default = cpal::SupportedStreamConfig::new(
            1,
            16_000,
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::I16,
        );
        assert!(usable_config(&default));
        let configs = fallback_configs(
            Some(&default),
            [
                range(1, 16_000, 16_000),
                range(2, 48_000, 48_000),
                range(1, 48_000, 48_000),
            ]
            .into_iter(),
        );
        assert_eq!(configs[0].channels(), 1);
        assert_eq!(configs[0].sample_rate(), 16_000);
        assert_eq!(configs.len(), 3);
    }

    #[test]
    fn discrete_rates_do_not_force_the_last_advertised_rate() {
        let default = range(2, 48_000, 48_000).with_sample_rate(48_000);
        let configs = fallback_configs(
            Some(&default),
            [
                range(2, 44_100, 44_100),
                range(2, 48_000, 48_000),
                range(2, 96_000, 96_000),
            ]
            .into_iter(),
        );
        // The default was already attempted; retain alternatives in rate order.
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].sample_rate(), 44_100);
        assert_eq!(configs[1].sample_rate(), 96_000);
    }

    #[test]
    fn fallback_without_default_accepts_mono_voice_rates_and_bounds_ranges() {
        for rate in [8_000, 16_000, 24_000, 32_000] {
            let configs = fallback_configs(None, [range(1, rate, rate)].into_iter());
            assert_eq!(configs.len(), 1);
            assert_eq!(configs[0].sample_rate(), rate);
            assert_eq!(configs[0].channels(), 1);
        }
        let default = range(2, 384_000, 384_000).with_sample_rate(384_000);
        let configs = fallback_configs(
            Some(&default),
            [
                range(0, 48_000, 48_000),
                range(9, 48_000, 48_000),
                range(1, 1_000, 4_000),
                range(2, 200_000, 384_000),
                range(1, 4_000, 384_000),
            ]
            .into_iter(),
        );
        assert!(!usable_config(&default));
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].sample_rate(), 192_000);
    }

    #[test]
    fn presentation_clock_accounts_for_device_latency_and_tail() {
        let clock = Clock::default();
        let mut presentation = PresentationClock::default();
        presentation.push(
            Presentation {
                start_ns: 1000,
                end_ns: 2000,
                first_frame: 0,
                frames: 100,
            },
            &clock,
        );
        presentation.advance(500, &clock);
        assert_eq!(clock.presented.load(Ordering::Relaxed), 0);
        presentation.advance(1500, &clock);
        assert_eq!(clock.presented.load(Ordering::Relaxed), 50);
        presentation.advance(2000, &clock);
        assert_eq!(clock.presented.load(Ordering::Relaxed), 100);
        assert_eq!(presentation.len, 0);
        presentation.advance(3000, &clock);
        assert_eq!(clock.presented.load(Ordering::Relaxed), 100);
    }
    #[test]
    fn callback_silences_underrun_and_freezes_pause() {
        let (mut p, mut c) = HeapRb::<f32>::new(8).split();
        let clock = Clock::default();
        clock.gain.store(0.5f32.to_bits(), Ordering::Relaxed);
        p.push_slice(&[1.0, -1.0, f32::NAN, 8.0]);
        let mut out = [9.0f32; 6];
        render(&mut out, &mut c, 2, &clock);
        assert_eq!(out, [0.0; 6]);
        assert_eq!(c.occupied_len(), 4);
        clock.playing.store(true, Ordering::Relaxed);
        render(&mut out, &mut c, 2, &clock);
        assert_eq!(out, [0.5, -0.5, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(clock.consumed.load(Ordering::Relaxed), 2);
        assert_eq!(clock.rendered.load(Ordering::Relaxed), 2);
        render(&mut out, &mut c, 2, &clock); // underrun silence is not listening
        assert_eq!(clock.rendered.load(Ordering::Relaxed), 2);
        clock.reset_request.store(1, Ordering::Release);
        assert!(reset_consumer(&mut c, &clock));
        assert_eq!(clock.consumed.load(Ordering::Relaxed), 0);
        assert_eq!(clock.rendered.load(Ordering::Relaxed), 2);
    }
}
