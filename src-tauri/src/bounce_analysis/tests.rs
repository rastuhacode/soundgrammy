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

fn mean(lane: &[u8]) -> f32 {
    lane.iter().map(|value| *value as f32).sum::<f32>() / lane.len() as f32
}

#[test]
fn bass_mid_and_treble_light_their_own_regions() {
    for (frequency, expected) in [(90.0, 0), (900.0, 1), (7_000.0, 2)] {
        let mut extractor = FeatureExtractor::new(48_000, 1);
        extractor.push_interleaved(&sine(0.4, frequency, 2.0));
        let profile = extractor.finish().unwrap();
        let bands = [&profile.bass, &profile.mid, &profile.treble];
        let dominant = mean(bands[expected]);
        assert!(
            dominant > 100.0,
            "frequency={frequency}, dominant={dominant}"
        );
        for (index, lane) in bands.iter().enumerate() {
            assert_eq!(lane.len(), profile.loudness.len());
            if index != expected {
                assert!(
                    mean(lane) < dominant * 0.15,
                    "frequency={frequency}, other band={index}, mean={}",
                    mean(lane)
                );
            }
        }
    }
}

#[test]
fn right_only_and_anti_phase_audio_keep_spectral_energy() {
    for anti_phase in [false, true] {
        let mono = sine(0.4, 90.0, 1.0);
        let stereo: Vec<f32> = mono
            .iter()
            .flat_map(|value| [if anti_phase { -*value } else { 0.0 }, *value])
            .collect();
        let mut extractor = FeatureExtractor::new(48_000, 2);
        extractor.push_interleaved(&stereo);
        let profile = extractor.finish().unwrap();
        assert!(mean(&profile.bass) > 100.0);
    }
}

#[test]
fn steady_compressed_master_stays_active_and_silence_stays_dark() {
    let mut extractor = FeatureExtractor::new(48_000, 1);
    extractor.push_interleaved(&sine(0.5, 90.0, 3.0));
    extractor.push_interleaved(&vec![0.0; 48_000]);
    let profile = extractor.finish().unwrap();
    assert!(mean(&profile.bass[40..100]) > 100.0);
    let tail = profile.bass.len() - 10;
    assert!(profile.bass[tail..].iter().all(|v| *v == 0));
    assert!(profile.treble[tail..].iter().all(|v| *v == 0));
    assert!(profile.loudness[tail..].iter().all(|v| *v == 0));
}

#[test]
fn response_is_stable_across_mastering_levels() {
    let analyze = |amplitude| {
        let mut extractor = FeatureExtractor::new(48_000, 1);
        extractor.push_interleaved(&sine(amplitude, 90.0, 1.0));
        extractor.push_interleaved(&sine(amplitude * 0.2, 90.0, 1.0));
        extractor.finish().unwrap()
    };
    let loud = analyze(0.8);
    let quiet = analyze(0.008);
    for (left, right) in loud.bass.iter().zip(quiet.bass) {
        assert!((*left as i32 - right as i32).abs() <= 2);
    }
}
