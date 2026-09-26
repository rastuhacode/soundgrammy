//! Stateful band-limited conversion. Delay is trimmed once, tails flushed once.
use super::output::sanitize;
use rubato::{FftFixedInOut, Resampler};

pub struct Converter {
    resampler: Option<FftFixedInOut<f32>>,
    pending: Vec<Vec<f32>>,
    channels: usize,
    input_rate: u32,
    output_rate: u32,
    input_frames: usize,
    emitted: usize,
    delay: usize,
}
impl Converter {
    pub fn new(input: u32, output: u32, channels: usize) -> Result<Self, &'static str> {
        if !(8_000..=192_000).contains(&input) || !(1..=8).contains(&channels) {
            return Err("unsupported-format");
        }
        let resampler = if input != output {
            Some(
                FftFixedInOut::new(input as usize, output as usize, 1024, channels)
                    .map_err(|_| "decode-failed")?,
            )
        } else {
            None
        };
        let delay = resampler.as_ref().map_or(0, |r| r.output_delay());
        Ok(Self {
            resampler,
            pending: vec![Vec::new(); channels],
            channels,
            input_rate: input,
            output_rate: output,
            input_frames: 0,
            emitted: 0,
            delay,
        })
    }
    pub fn push(
        &mut self,
        samples: &[f32],
        input_channels: usize,
        finish: bool,
    ) -> Result<Vec<f32>, &'static str> {
        if !(1..=2).contains(&input_channels) {
            return Err("unsupported-format");
        }
        for frame in samples.chunks_exact(input_channels) {
            let left = sanitize(frame[0]);
            let right = sanitize(frame[input_channels - 1]);
            for channel in 0..self.channels {
                self.pending[channel].push(if self.channels == 1 {
                    (left + right) * 0.5
                } else if channel == 0 {
                    left
                } else if channel == 1 {
                    right
                } else {
                    0.0
                });
            }
            self.input_frames += 1;
        }
        let target =
            (self.input_frames as u64 * self.output_rate as u64 / self.input_rate as u64) as usize;
        let mut result = Vec::new();
        loop {
            let chunk = self
                .resampler
                .as_ref()
                .map_or(self.pending[0].len(), |r| r.input_frames_next());
            if chunk == 0
                || (!finish && self.pending[0].len() < chunk)
                || (finish && self.emitted >= target)
            {
                break;
            }
            let mut input = vec![vec![0.0; chunk]; self.channels];
            let available = self.pending[0].len().min(chunk);
            for (source, dest) in self.pending.iter_mut().zip(&mut input) {
                dest[..available].copy_from_slice(&source[..available]);
                source.drain(..available);
            }
            let output = match self.resampler.as_mut() {
                Some(r) => r.process(&input, None).map_err(|_| "decode-failed")?,
                None => input,
            };
            for frame in 0..output[0].len() {
                if self.delay > 0 {
                    self.delay -= 1;
                    continue;
                }
                if finish && self.emitted >= target {
                    break;
                }
                for channel in &output {
                    result.push(sanitize(channel[frame]));
                }
                self.emitted += 1;
            }
            if !finish && self.resampler.is_none() {
                break;
            }
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_output_mixes_both_stereo_channels() {
        let mut converter = Converter::new(48_000, 48_000, 1).unwrap();
        let mono = converter
            .push(&[1.0, 0.0, 0.0, 1.0, 1.0, -1.0], 2, true)
            .unwrap();
        assert_eq!(mono, [0.5, 0.5, 0.0]);
    }

    #[test]
    fn stereo_music_resamples_to_mono_headset_rates() {
        for input in [44_100, 48_000] {
            let stereo: Vec<f32> = (0..input)
                .flat_map(|i| [(i as f32 * 0.03).sin() * 0.5, 0.0])
                .collect();
            let mono: Vec<f32> = stereo.chunks_exact(2).map(|f| f[0] * 0.5).collect();
            for rate in [8_000, 16_000, 24_000, 32_000] {
                let mut converter = Converter::new(input, rate, 1).unwrap();
                let mut result = Vec::new();
                for packet in stereo.chunks(634) {
                    result.extend(converter.push(packet, 2, false).unwrap());
                }
                result.extend(converter.push(&[], 2, true).unwrap());
                let mut reference = Converter::new(input, rate, 1).unwrap();
                let expected = reference.push(&mono, 1, true).unwrap();
                assert_eq!(result.len(), rate as usize);
                assert_eq!(result, expected);
                assert!(result.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
                assert!(result.iter().any(|s| s.abs() > 0.1));
            }
        }
    }

    #[test]
    fn resampling_is_packet_independent_and_has_exact_duration() {
        for (input, output) in [(44100, 48000), (48000, 44100)] {
            let samples: Vec<f32> = (0..input).map(|i| (i as f32 * 0.03).sin() * 0.5).collect();
            let mut a = Converter::new(input, output, 2).unwrap();
            let mut whole = a.push(&samples, 1, false).unwrap();
            whole.extend(a.push(&[], 1, true).unwrap());
            let mut b = Converter::new(input, output, 2).unwrap();
            let mut split = Vec::new();
            for packet in samples.chunks(317) {
                split.extend(b.push(packet, 1, false).unwrap());
            }
            split.extend(b.push(&[], 1, true).unwrap());
            assert_eq!(whole.len(), output as usize * 2);
            assert_eq!(whole, split);
            assert!(whole.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!(whole.as_chunks::<2>().0.iter().all(|f| f[0] == f[1]));
        }
    }
}
