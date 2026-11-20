//! Frequency-band motion from develop's artwork-bounce policy. Profile work is
//! done once when analysis arrives; animation samples a fixed number of lanes.
#[derive(PartialEq)]
pub(super) struct BounceProfile {
    pub frame_ms: i64,
    pub loudness: Vec<u8>,
    pub bands: [Vec<u8>; 3],
    transients: [Vec<u8>; 3],
    density: Vec<u8>,
    bass_density: Vec<u8>,
    reference: f64,
    boosts: [f64; 3],
}
fn percentile(values: &[u8], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted
        .get(((sorted.len().saturating_sub(1)) as f64 * q) as usize)
        .copied()
        .unwrap_or(0) as f64
}
fn transients(lane: &[u8], frame_ms: i64) -> Vec<u8> {
    let lookback = (75. / frame_ms as f64).round().max(1.) as usize;
    let changes: Vec<f64> = lane
        .iter()
        .enumerate()
        .map(|(i, v)| {
            ((*v as f64 - lane[i.saturating_sub(lookback)] as f64) / 255. - 0.025).max(0.)
        })
        .collect();
    let mut positive: Vec<f64> = changes.iter().copied().filter(|v| *v > 0.).collect();
    positive.sort_by(f64::total_cmp);
    let reference = positive
        .get(((positive.len().saturating_sub(1)) as f64 * 0.9) as usize)
        .copied()
        .unwrap_or(0.)
        .max(0.18);
    changes
        .iter()
        .zip(lane)
        .map(|(change, level)| {
            (255. * (change / reference).min(1.) * (*level as f64 / 64.).min(1.)).round() as u8
        })
        .collect()
}
fn density(lane: &[u8], frame_ms: i64) -> Vec<u8> {
    let radius = (1000. / frame_ms as f64).round().max(1.) as usize;
    let mut prefix = Vec::with_capacity(lane.len() + 1);
    prefix.push(0usize);
    for v in lane {
        prefix.push(prefix.last().unwrap() + usize::from(*v > 48));
    }
    (0..lane.len())
        .map(|i| {
            let start = i.saturating_sub(radius);
            let end = (i + radius + 1).min(lane.len());
            (255. * (prefix[end] - prefix[start]) as f64 / (end - start) as f64).round() as u8
        })
        .collect()
}
pub fn sample(lane: &[u8], seconds: f64, frame_ms: i64) -> f64 {
    if lane.is_empty() || frame_ms <= 0 || !seconds.is_finite() {
        return 0.;
    }
    let position = (seconds * 1000. / frame_ms as f64).max(0.);
    if position >= lane.len() as f64 {
        return 0.;
    }
    let left = position.floor() as usize;
    let right = (left + 1).min(lane.len() - 1);
    let mix = position.fract();
    (lane[left] as f64 * (1. - mix) + lane[right] as f64 * mix) / 255.
}
impl BounceProfile {
    pub fn new(
        frame_ms: i64,
        loudness: Vec<u8>,
        onset: Vec<u8>,
        bands: [Vec<u8>; 3],
    ) -> Option<Self> {
        if frame_ms <= 0
            || loudness.is_empty()
            || onset.len() != loudness.len()
            || bands.iter().any(|b| b.len() != loudness.len())
        {
            return None;
        }
        let transients = bands.each_ref().map(|b| transients(b, frame_ms));
        let peak = loudness.iter().copied().max().unwrap_or(1).max(1) as f64;
        let reference = (peak * 0.94).min((peak * 0.85).max(percentile(&loudness, 0.95)));
        let loud: Vec<usize> = loudness
            .iter()
            .enumerate()
            .filter(|(_, v)| **v as f64 >= reference * 0.9)
            .map(|(i, _)| i)
            .collect();
        let boosts = std::array::from_fn(|band| {
            if band == 0 {
                return 0.;
            }
            let level = percentile(
                &loud.iter().map(|i| bands[band][*i]).collect::<Vec<_>>(),
                0.5,
            ) / 255.;
            let attack = percentile(
                &loud
                    .iter()
                    .map(|i| transients[band][*i])
                    .collect::<Vec<_>>(),
                0.9,
            ) / 255.;
            ((level - 0.72) / 0.10).clamp(0., 1.) * (0.30 / attack.max(0.16) - 1.).clamp(0., 0.75)
        });
        Some(Self {
            frame_ms,
            density: density(&onset, frame_ms),
            bass_density: density(&transients[0], frame_ms),
            loudness,
            bands,
            transients,
            reference: reference / 255.,
            boosts,
        })
    }
}
fn follow(current: f64, target: f64, elapsed: f64, attack: f64, release: f64) -> f64 {
    current
        + (target - current)
            * (1. - (-elapsed.max(0.) / if target > current { attack } else { release }).exp())
}
#[derive(Default)]
pub(super) struct Motion {
    body: f64,
    bass: f64,
    mid: f64,
    treble: f64,
    quiet_ms: f64,
    handoff: f64,
    band_body: [f64; 3],
    band_pulse: [f64; 3],
    band_trail: [f64; 3],
    pub energy: [f64; 3],
    pub spread: [f64; 3],
}
impl Motion {
    pub fn sample(
        &mut self,
        profile: &BounceProfile,
        seconds: f64,
        playing: bool,
        fade: f64,
        elapsed: f64,
    ) -> (f32, f32) {
        let sample = |lane: &[u8]| super::bounce::sample(lane, seconds, profile.frame_ms);
        let near_peak = if playing {
            (sample(&profile.loudness) / profile.reference).min(1.)
        } else {
            0.
        };
        let gain = near_peak.powi(4) * fade;
        let palette_gain = (0.82 + 0.18 * near_peak) * fade;
        let activity = sample(&profile.density);
        let bass_activity = sample(&profile.bass_density);
        let elapsed = elapsed.clamp(0., 50.);
        let release = if playing { 1. } else { 0.3 };
        for i in 0..3 {
            let level = if playing {
                sample(&profile.bands[i]) * palette_gain
            } else {
                0.
            };
            let attack = if playing {
                sample(&profile.transients[i])
            } else {
                0.
            };
            let accent = (attack + profile.boosts[i] * near_peak.powi(2) * attack * (1. - attack))
                * palette_gain;
            self.band_body[i] = follow(self.band_body[i], level, elapsed, 120., 700. * release);
            self.band_pulse[i] = follow(
                self.band_pulse[i],
                accent,
                elapsed,
                14.,
                (180. - activity * 50.) * release,
            );
            let (attack, decay) = [(160., 880.), (125., 720.), (90., 500.)][i];
            self.band_trail[i] = follow(
                self.band_trail[i],
                self.band_pulse[i],
                elapsed,
                attack,
                decay * release,
            );
        }
        let level = |i: usize| {
            if playing {
                sample(&profile.bands[i])
            } else {
                0.
            }
        };
        let attack = |i: usize| {
            if playing {
                sample(&profile.transients[i])
            } else {
                0.
            }
        };
        let bass_level = level(0);
        let bass_attack = attack(0);
        self.quiet_ms = if playing && bass_attack < 0.20 {
            (self.quiet_ms + elapsed).min(1400.)
        } else {
            0.
        };
        self.handoff = follow(
            self.handoff,
            ((self.quiet_ms - 550.) / 850.).max(0.),
            elapsed,
            100.,
            70.,
        );
        self.body = follow(
            self.body,
            gain * bass_level.max(level(1) * 0.12),
            elapsed,
            180.,
            550.,
        );
        let accent = bass_attack + 0.45 * bass_level * bass_attack * (1. - bass_attack);
        let target = accent * gain / (1. + 0.65 * bass_activity * (1. - gain));
        self.bass = follow(
            self.bass,
            target,
            elapsed,
            30. + 10. * bass_activity,
            205. - 60. * bass_activity,
        );
        self.mid = follow(
            self.mid,
            gain * attack(1) * (0.33 + 0.27 * self.handoff),
            elapsed,
            65.,
            275.,
        );
        self.treble = follow(
            self.treble,
            gain * attack(2) * (0.13 + 0.09 * self.handoff),
            elapsed,
            95.,
            330.,
        );
        let impact = self.bass + (1. - self.bass).powi(2) * self.mid.max(self.treble);
        let spill = self
            .band_trail
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            * 0.24;
        for i in 0..3 {
            let color =
                self.band_body[i] * 0.14 + self.band_pulse[i] * 0.8 + self.band_trail[i] * 0.22;
            self.energy[i] = color - (color - 0.74).max(0.) * (0.16 / 0.42);
            self.spread[i] =
                (self.band_body[i] * 0.15 + self.band_pulse[i] * 0.1 + self.band_trail[i] + spill)
                    .min(1.);
        }
        (
            (1. + self.body * 0.02 + impact * 0.055) as f32,
            (impact * 6.1) as f32,
        )
    }
    pub fn active(&self) -> bool {
        self.body
            .max(self.bass)
            .max(self.mid)
            .max(self.treble)
            .max(self.energy.iter().copied().fold(0., f64::max))
            > 0.001
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_end_is_silent() {
        assert_eq!(sample(&[255; 4], 1., 25), 0.);
        assert_eq!(sample(&[0, 255], 0.0125, 25), 0.5);
    }
    #[test]
    fn sustained_bass_has_no_synthetic_attacks() {
        assert!(transients(&[220; 100], 25).iter().all(|v| *v == 0));
    }
    #[test]
    fn bands_react_independently_and_pause_releases() {
        let n = 160;
        let mut bass = vec![0; n];
        for (i, v) in bass.iter_mut().enumerate() {
            if i % 16 < 4 {
                *v = 255;
            }
        }
        let p = BounceProfile::new(
            25,
            vec![255; n],
            bass.clone(),
            [bass, vec![0; n], vec![0; n]],
        )
        .unwrap();
        let mut motion = Motion::default();
        let mut peak = 1f32;
        for i in 0..n {
            let (scale, _) = motion.sample(&p, i as f64 * 0.025, true, 1., 25.);
            peak = peak.max(scale);
        }
        assert!(peak > 1.01);
        assert!(motion.energy[0] > 0.);
        assert_eq!(motion.energy[1], 0.);
        for _ in 0..200 {
            motion.sample(&p, 0., false, 1., 25.);
        }
        assert!(!motion.active());
    }
}
