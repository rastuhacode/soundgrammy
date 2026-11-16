use std::collections::VecDeque;

use super::FRAME_MS;

/// Track-relative contrast with a slow local reference. A minimum dB span keeps
/// compressed masters alive without stretching tiny fluctuations into full hits.
fn normalize_energy(power: &[f32], attack_ms: f32, release_ms: f32) -> Vec<f32> {
    let mut envelope = Vec::with_capacity(power.len());
    let mut current = 0.0_f32;
    for value in power {
        let timing = if *value > current {
            attack_ms
        } else {
            release_ms
        };
        current += (*value - current) * (1.0 - (-FRAME_MS as f32 / timing).exp());
        envelope.push(current.max(1.0e-12).log10() * 10.0);
    }
    let mut active: Vec<f32> = envelope.iter().copied().filter(|db| *db > -100.0).collect();
    if active.is_empty() {
        return vec![0.0; power.len()];
    }
    active.sort_by(f32::total_cmp);
    let high = percentile(&active, 0.95);
    let low = percentile(&active, 0.10).min(high - 18.0);
    let mut reference = envelope[0];
    let local_alpha = 1.0 - (-FRAME_MS as f32 / 4_000.0).exp();
    envelope
        .iter()
        .zip(power)
        .map(|(db, raw)| {
            reference += (*db - reference) * local_alpha;
            let global = ((*db - low) / (high - low)).clamp(0.0, 1.0);
            let local = (0.45 + (*db - reference) / 24.0).clamp(0.0, 1.0);
            // Preserve rests and quiet passages; local gain must not promote silence.
            let gate = ((10.0 * raw.max(1.0e-12).log10() - (high - 54.0)) / 12.0).clamp(0.0, 1.0);
            (global * 0.8 + local * global.sqrt() * 0.2) * gate
        })
        .collect()
}

pub(super) fn normalize_loudness(power: &[f32]) -> Vec<f32> {
    normalize_energy(power, 100.0, 550.0)
}

pub(super) fn normalize_band(power: &[f32], broadband: &[f32]) -> Vec<f32> {
    let mut values = normalize_energy(power, 25.0, 120.0);
    for ((value, band), total) in values.iter_mut().zip(power).zip(broadband) {
        // Do not independently amplify FFT leakage or a nearly absent band.
        let share = (*band / total.max(1.0e-12)).clamp(0.0, 1.0);
        *value *= (share / 0.04).clamp(0.0, 1.0).sqrt();
    }
    values
}

pub(super) fn normalize_onset(onset: &[f32], power: &[f32]) -> Vec<f32> {
    let radius = (500 / FRAME_MS) as usize;
    let peak_power = power.iter().copied().fold(0.0_f32, f32::max);
    let mut residual = vec![0.0; onset.len()];
    for index in 0..onset.len() {
        let start = index.saturating_sub(radius);
        let end = (index + radius + 1).min(onset.len());
        let mut local = onset[start..end].to_vec();
        local.sort_by(f32::total_cmp);
        residual[index] = (onset[index] - percentile(&local, 0.5)).max(0.0);
        if power.get(index).copied().unwrap_or(0.0) < (peak_power * 1.0e-5).max(1.0e-12) {
            residual[index] = 0.0;
        }
    }
    let mut positive: Vec<f32> = residual.iter().copied().filter(|v| *v > 1.0e-9).collect();
    if positive.is_empty() {
        return residual;
    }
    positive.sort_by(f32::total_cmp);
    let ceiling = percentile(&positive, 0.95).max(1.0e-9);
    let mut window = VecDeque::new();
    let mut sum = 0.0;
    let window_frames = (2_000 / FRAME_MS) as usize;
    residual
        .iter()
        .map(|value| {
            let normalized = (*value / ceiling).clamp(0.0, 1.0);
            window.push_back(normalized);
            sum += normalized;
            if window.len() > window_frames {
                sum -= window.pop_front().unwrap_or(0.0);
            }
            // Dense percussion spends less time at the ceiling than isolated hits.
            normalized / (1.0 + 0.8 * sum / window.len() as f32)
        })
        .collect()
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    sorted
        .get(((sorted.len().saturating_sub(1)) as f32 * q).round() as usize)
        .copied()
        .unwrap_or(0.0)
}

pub(super) fn quantize(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}
