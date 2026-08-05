//! Whole-track, genre-agnostic motion profiles for fullscreen artwork.

use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::Ordering;

use base64::Engine;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use serde::Serialize;
use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::{well_known::CODEC_ID_OPUS, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::cache;
use crate::db::TrackBounceProfileRecord;
use crate::state::AppState;

pub const ALGORITHM_VERSION: i64 = 1;
pub const FRAME_MS: i64 = 50;
const FFT_SIZE: usize = 2048;
const FFT_HOP: usize = 1024;

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum BounceProfileResponse {
    Ready {
        #[serde(rename = "algorithmVersion")]
        algorithm_version: i64,
        #[serde(rename = "frameMs")]
        frame_ms: i64,
        #[serde(rename = "durationMs")]
        duration_ms: i64,
        #[serde(rename = "loudnessData")]
        loudness_data: String,
        #[serde(rename = "onsetData")]
        onset_data: String,
    },
    Unavailable,
}

impl BounceProfileResponse {
    fn from_record(record: TrackBounceProfileRecord) -> Self {
        let encoder = base64::engine::general_purpose::STANDARD;
        Self::Ready {
            algorithm_version: record.algorithm_version,
            frame_ms: record.frame_ms,
            duration_ms: record.duration_ms,
            loudness_data: encoder.encode(record.loudness),
            onset_data: encoder.encode(record.onset),
        }
    }
}

pub async fn profile_for_track(
    state: &AppState,
    app: &tauri::AppHandle,
    track_id: i64,
) -> BounceProfileResponse {
    state
        .bounce_requested_track
        .store(track_id, Ordering::Relaxed);
    let track = match cache::require_track(state, track_id) {
        Ok(track) => track,
        Err(_) => return BounceProfileResponse::Unavailable,
    };

    if let Ok(Some(record)) = state.db.track_bounce_profile(track_id) {
        if record.algorithm_version == ALGORITHM_VERSION && record.file_size == track.file_size {
            return BounceProfileResponse::from_record(record);
        }
    }

    let track_lock = state.bounce_lock_for(track_id).await;
    let _track_guard = track_lock.lock().await;
    if let Ok(Some(record)) = state.db.track_bounce_profile(track_id) {
        if record.algorithm_version == ALGORITHM_VERSION && record.file_size == track.file_size {
            return BounceProfileResponse::from_record(record);
        }
    }

    // Playback owns the same deduplicated download, so waiting here never
    // blocks entering fullscreen or the media element itself.
    let path = match cache::ensure_audio(state, app, track_id).await {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(track_id, "bounce profile download unavailable: {error}");
            return BounceProfileResponse::Unavailable;
        }
    };

    let _analysis_guard = state.bounce_analysis_slot.lock().await;
    if state.bounce_requested_track.load(Ordering::Relaxed) != track_id {
        return BounceProfileResponse::Unavailable;
    }
    let result = tauri::async_runtime::spawn_blocking(move || analyze_file(&path)).await;
    let analyzed = match result {
        Ok(Ok(profile)) => profile,
        Ok(Err(error)) => {
            tracing::warn!(track_id, "bounce profile analysis failed: {error}");
            return BounceProfileResponse::Unavailable;
        }
        Err(error) => {
            tracing::warn!(track_id, "bounce profile worker failed: {error}");
            return BounceProfileResponse::Unavailable;
        }
    };

    let record = TrackBounceProfileRecord {
        track_id,
        algorithm_version: ALGORITHM_VERSION,
        frame_ms: FRAME_MS,
        duration_ms: analyzed.duration_ms,
        file_size: track.file_size,
        loudness: analyzed.loudness,
        onset: analyzed.onset,
    };
    if let Err(error) = state.db.save_track_bounce_profile(&record) {
        tracing::warn!(track_id, "could not cache bounce profile: {error}");
    }
    BounceProfileResponse::from_record(record)
}

#[derive(Debug)]
struct AnalyzedProfile {
    duration_ms: i64,
    loudness: Vec<u8>,
    onset: Vec<u8>,
}

fn analyze_file(path: &Path) -> anyhow::Result<AnalyzedProfile> {
    let file = Box::new(File::open(path)?);
    let stream = MediaSourceStream::new(file, Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe().probe(
        &hint,
        stream,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow::anyhow!("file has no audio track"))?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| anyhow::anyhow!("audio track has no codec parameters"))?
        .clone();
    let track_id = track.id;
    let channels = params
        .channels
        .as_ref()
        .map(|value| value.count())
        .unwrap_or(2)
        .max(1);
    let is_opus = params.codec == CODEC_ID_OPUS;
    if is_opus && channels > 2 {
        anyhow::bail!("multichannel Opus profiles are not supported");
    }
    let sample_rate = if is_opus {
        48_000
    } else {
        params.sample_rate.unwrap_or(48_000)
    };
    let mut extractor = FeatureExtractor::new(sample_rate, channels);

    if is_opus {
        let mut decoder = ruopus::OpusDecoder::new(channels);
        while let Some(packet) = format.next_packet()? {
            if packet.track_id != track_id {
                continue;
            }
            match decoder.decode_packet(&packet.data) {
                Ok(samples) => extractor.push_interleaved(&samples),
                Err(error) => tracing::debug!("skipping malformed Opus packet: {error}"),
            }
        }
    } else {
        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())?;
        let mut samples = Vec::<f32>::new();
        loop {
            let packet = match format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => break,
                Err(SymphoniaError::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            if packet.track_id != track_id {
                continue;
            }
            match decoder.decode(&packet) {
                Ok(buffer) => {
                    samples.resize(buffer.samples_interleaved(), f32::MID);
                    buffer.copy_to_slice_interleaved(&mut samples);
                    extractor.push_interleaved(&samples);
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(SymphoniaError::ResetRequired) => decoder.reset(),
                Err(error) => return Err(error.into()),
            }
        }
    }

    extractor.finish()
}

#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    fn process(self, state: &mut BiquadState, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * state.x1 + self.b2 * state.x2
            - self.a1 * state.y1
            - self.a2 * state.y2;
        state.x2 = state.x1;
        state.x1 = input;
        state.y2 = state.y1;
        state.y1 = output;
        output
    }

    fn high_pass(sample_rate: f32, frequency: f32, q: f32) -> Self {
        let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let cos = omega.cos();
        let alpha = omega.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos) / 2.0) / a0,
            b1: (-(1.0 + cos)) / a0,
            b2: ((1.0 + cos) / 2.0) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    fn high_shelf(sample_rate: f32, frequency: f32, gain_db: f32) -> Self {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let cos = omega.cos();
        let sin = omega.sin();
        let alpha = sin / 2.0 * 2.0_f32.sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        let a0 = (a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha;
        Self {
            b0: a * ((a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha) / a0,
            b1: -2.0 * a * ((a - 1.0) + (a + 1.0) * cos) / a0,
            b2: a * ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha) / a0,
            a1: 2.0 * ((a - 1.0) - (a + 1.0) * cos) / a0,
            a2: ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha) / a0,
        }
    }
}

struct FeatureExtractor {
    sample_rate: usize,
    channels: usize,
    frame_samples: usize,
    total_frames: usize,
    frame_power: f64,
    frame_count: usize,
    power_frames: Vec<f32>,
    onset_sum: Vec<f32>,
    onset_count: Vec<u16>,
    shelf: Biquad,
    high_pass: Biquad,
    shelf_state: Vec<BiquadState>,
    high_pass_state: Vec<BiquadState>,
    fft: std::sync::Arc<dyn Fft<f32>>,
    fft_window: Vec<f32>,
    fft_buffer: VecDeque<f32>,
    fft_scratch: Vec<Complex32>,
    previous_spectrum: Vec<f32>,
    samples_until_fft: usize,
}

impl FeatureExtractor {
    fn new(sample_rate: u32, channels: usize) -> Self {
        let sample_rate = sample_rate as usize;
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let fft_window = (0..FFT_SIZE)
            .map(|index| {
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * index as f32 / FFT_SIZE as f32).cos()
            })
            .collect();
        Self {
            sample_rate,
            channels,
            frame_samples: (sample_rate * FRAME_MS as usize / 1000).max(1),
            total_frames: 0,
            frame_power: 0.0,
            frame_count: 0,
            power_frames: Vec::new(),
            onset_sum: Vec::new(),
            onset_count: Vec::new(),
            shelf: Biquad::high_shelf(sample_rate as f32, 1_500.0, 4.0),
            high_pass: Biquad::high_pass(sample_rate as f32, 60.0, 0.5),
            shelf_state: vec![BiquadState::default(); channels],
            high_pass_state: vec![BiquadState::default(); channels],
            fft,
            fft_window,
            fft_buffer: VecDeque::with_capacity(FFT_SIZE * 2),
            fft_scratch: vec![Complex32::default(); FFT_SIZE],
            previous_spectrum: vec![0.0; FFT_SIZE / 2],
            samples_until_fft: FFT_SIZE,
        }
    }

    fn push_interleaved(&mut self, samples: &[f32]) {
        for frame in samples.chunks_exact(self.channels) {
            let mut power = 0.0_f64;
            for (channel, sample) in frame.iter().enumerate() {
                let weighted = self.shelf.process(&mut self.shelf_state[channel], *sample);
                let weighted = self
                    .high_pass
                    .process(&mut self.high_pass_state[channel], weighted);
                power += f64::from(weighted) * f64::from(weighted);
            }
            self.frame_power += power / self.channels as f64;
            self.frame_count += 1;
            self.total_frames += 1;

            // A stable channel is preferable to averaging here: anti-phase
            // stereo should not erase spectral motion.
            self.fft_buffer.push_back(frame[0]);
            self.samples_until_fft = self.samples_until_fft.saturating_sub(1);
            if self.samples_until_fft == 0 && self.fft_buffer.len() >= FFT_SIZE {
                self.compute_flux();
                self.samples_until_fft = FFT_HOP;
                for _ in 0..FFT_HOP.min(self.fft_buffer.len()) {
                    self.fft_buffer.pop_front();
                }
            }

            if self.frame_count == self.frame_samples {
                self.flush_power_frame();
            }
        }
    }

    fn flush_power_frame(&mut self) {
        if self.frame_count == 0 {
            return;
        }
        self.power_frames
            .push((self.frame_power / self.frame_count as f64) as f32);
        self.frame_power = 0.0;
        self.frame_count = 0;
    }

    fn compute_flux(&mut self) {
        for (index, value) in self.fft_buffer.iter().take(FFT_SIZE).enumerate() {
            self.fft_scratch[index] = Complex32::new(*value * self.fft_window[index], 0.0);
        }
        self.fft.process(&mut self.fft_scratch);
        let hz_per_bin = self.sample_rate as f32 / FFT_SIZE as f32;
        let mut flux = 0.0;
        for index in 1..FFT_SIZE / 2 {
            let frequency = index as f32 * hz_per_bin;
            if !(45.0..=16_000.0).contains(&frequency) {
                continue;
            }
            let magnitude = self.fft_scratch[index].norm().ln_1p();
            let delta = (magnitude - self.previous_spectrum[index]).max(0.0);
            let weight = if frequency < 180.0 {
                1.2
            } else if frequency < 2_500.0 {
                1.0
            } else {
                0.75
            };
            flux += delta * weight;
            self.previous_spectrum[index] = magnitude;
        }
        let sample_position = self.total_frames.saturating_sub(FFT_SIZE / 2);
        let profile_index = sample_position / self.frame_samples;
        if self.onset_sum.len() <= profile_index {
            self.onset_sum.resize(profile_index + 1, 0.0);
            self.onset_count.resize(profile_index + 1, 0);
        }
        self.onset_sum[profile_index] += flux;
        self.onset_count[profile_index] = self.onset_count[profile_index].saturating_add(1);
    }

    fn finish(mut self) -> anyhow::Result<AnalyzedProfile> {
        self.flush_power_frame();
        if self.power_frames.is_empty() {
            anyhow::bail!("decoded track contains no samples");
        }
        let frame_count = self.power_frames.len();
        let mut raw_onset = vec![0.0; frame_count];
        for (index, output) in raw_onset.iter_mut().enumerate() {
            if let Some(count) = self
                .onset_count
                .get(index)
                .copied()
                .filter(|count| *count > 0)
            {
                *output = self.onset_sum[index] / count as f32;
            }
        }
        let loudness = normalize_loudness(&self.power_frames);
        let onset = normalize_onset(&raw_onset, &self.power_frames);
        Ok(AnalyzedProfile {
            duration_ms: ((self.total_frames as u128 * 1000) / self.sample_rate as u128) as i64,
            loudness: quantize(&loudness),
            onset: quantize(&onset),
        })
    }
}

fn normalize_loudness(power: &[f32]) -> Vec<f32> {
    let mut momentary = Vec::with_capacity(power.len());
    let mut window = VecDeque::with_capacity(8);
    let mut sum = 0.0_f32;
    for value in power {
        window.push_back(*value);
        sum += *value;
        if window.len() > 8 {
            sum -= window.pop_front().unwrap_or(0.0);
        }
        momentary.push(sum / window.len() as f32);
    }
    let alpha = 1.0 - (-FRAME_MS as f32 / 2_000.0).exp();
    let mut smoothed = Vec::with_capacity(momentary.len());
    let mut current = 0.0;
    for value in momentary {
        current += (value - current) * alpha;
        smoothed.push(if current > 1.0e-6 {
            10.0 * current.log10()
        } else {
            -60.0
        });
    }
    robust_normalize(&smoothed, -60.0, 0.10, 0.95)
}

fn normalize_onset(onset: &[f32], power: &[f32]) -> Vec<f32> {
    let mut residual = vec![0.0; onset.len()];
    for index in 0..onset.len() {
        let start = index.saturating_sub(10);
        let end = (index + 11).min(onset.len());
        let mut local = onset[start..end].to_vec();
        local.sort_by(f32::total_cmp);
        let median = local.get(local.len() / 2).copied().unwrap_or(0.0);
        residual[index] = (onset[index] - median).max(0.0);
        if power.get(index).copied().unwrap_or(0.0) < 1.0e-7 {
            residual[index] = 0.0;
        }
    }
    robust_normalize(&residual, 0.0, 0.50, 0.98)
}

fn robust_normalize(values: &[f32], floor: f32, low_q: f32, high_q: f32) -> Vec<f32> {
    let mut sorted: Vec<f32> = values
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > floor)
        .collect();
    if sorted.is_empty() {
        return vec![0.0; values.len()];
    }
    sorted.sort_by(f32::total_cmp);
    let at = |q: f32| {
        let index = ((sorted.len() - 1) as f32 * q).round() as usize;
        sorted[index]
    };
    let low = at(low_q);
    let high = at(high_q);
    let span = (high - low).max(1.0e-6);
    values
        .iter()
        .map(|value| {
            let normalized = ((*value - low) / span).clamp(0.0, 1.0);
            normalized * normalized * (3.0 - 2.0 * normalized)
        })
        .collect()
}

fn quantize(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amplitude: f32, frequency: f32, seconds: f32) -> Vec<f32> {
        let sample_rate = 48_000.0;
        (0..(seconds * sample_rate) as usize)
            .map(|index| {
                amplitude
                    * (2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate).sin()
            })
            .collect()
    }

    #[test]
    fn loud_sustained_section_outweighs_quiet_intro() {
        let mut extractor = FeatureExtractor::new(48_000, 1);
        extractor.push_interleaved(&sine(0.04, 220.0, 3.0));
        extractor.push_interleaved(&sine(0.75, 220.0, 3.0));
        let profile = extractor.finish().unwrap();
        let midpoint = profile.loudness.len() / 2;
        let intro = profile.loudness[..midpoint]
            .iter()
            .map(|v| *v as f32)
            .sum::<f32>()
            / midpoint as f32;
        let drop = profile.loudness[midpoint..]
            .iter()
            .map(|v| *v as f32)
            .sum::<f32>()
            / (profile.loudness.len() - midpoint) as f32;
        assert!(drop > intro + 60.0, "intro={intro}, drop={drop}");
    }

    #[test]
    fn anti_phase_stereo_retains_loudness() {
        let mono = sine(0.5, 440.0, 1.0);
        let mut stereo = Vec::with_capacity(mono.len() * 2);
        for sample in mono {
            stereo.extend([sample, -sample]);
        }
        let mut extractor = FeatureExtractor::new(48_000, 2);
        extractor.push_interleaved(&stereo);
        let profile = extractor.finish().unwrap();
        assert!(profile.loudness.iter().any(|value| *value > 0));
    }

    #[test]
    fn spectral_change_produces_onset_energy_at_constant_level() {
        let mut samples = sine(0.4, 180.0, 1.5);
        samples.extend(sine(0.4, 2_400.0, 1.5));
        let mut extractor = FeatureExtractor::new(48_000, 1);
        extractor.push_interleaved(&samples);
        let profile = extractor.finish().unwrap();
        assert!(profile.onset.iter().any(|value| *value > 128));
        let active = profile.onset.iter().filter(|value| **value > 32).count();
        assert!(active < profile.onset.len() / 2);
    }

    #[test]
    fn silence_is_zero_and_short_input_is_safe() {
        let mut extractor = FeatureExtractor::new(48_000, 1);
        extractor.push_interleaved(&vec![0.0; 4_800]);
        let profile = extractor.finish().unwrap();
        assert!(profile.loudness.iter().all(|value| *value == 0));
        assert!(profile.onset.iter().all(|value| *value == 0));
    }

    #[test]
    fn quantization_stays_bounded() {
        assert_eq!(quantize(&[-1.0, 0.5, 2.0]), vec![0, 128, 255]);
    }
}
