use super::normalization::quantize;
use super::FeatureExtractor;

fn sine(amplitude: f32, frequency: f32, seconds: f32) -> Vec<f32> {
    let sample_rate = 48_000.0;
    (0..(seconds * sample_rate) as usize)
        .map(|index| {
            amplitude * (2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate).sin()
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
