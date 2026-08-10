use std::collections::VecDeque;

use super::FRAME_MS;

pub(super) fn normalize_loudness(power: &[f32]) -> Vec<f32> {
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

pub(super) fn normalize_onset(onset: &[f32], power: &[f32]) -> Vec<f32> {
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

pub(super) fn quantize(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}
