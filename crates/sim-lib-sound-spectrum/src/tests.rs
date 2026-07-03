use std::time::Duration;

use super::*;
use sim_lib_sound_core::{Amplitude, Frequency, Tone};

#[test]
fn spectrum_peaks_recover_sine_frequency() {
    let tone = Tone::sine(Frequency(440.0), Duration::from_secs(1));
    let spectrum = Spectrum::from_tone(&tone, Duration::from_millis(100));
    let peak = spectrum.peaks(1)[0].0;
    assert!((peak.0 - 440.0).abs() < 1e-6);
}

#[test]
fn centroid_rises_with_brighter_partials() {
    let base = Spectrum {
        bins: vec![
            (Frequency(220.0), Amplitude(1.0)),
            (Frequency(880.0), Amplitude(0.1)),
        ],
        source: SpectrumSource::Synthetic,
    };
    let brighter = Spectrum {
        bins: vec![
            (Frequency(220.0), Amplitude(1.0)),
            (Frequency(880.0), Amplitude(0.8)),
        ],
        source: SpectrumSource::Synthetic,
    };
    assert!(brighter.centroid().0 > base.centroid().0);
}

#[test]
fn pcm_sine_peak_is_near_expected_bin() {
    let sample_rate = 8_000u32;
    let window = 256usize;
    let frequency = 437.5;
    let samples: Vec<f32> = (0..window)
        .map(|index| {
            let phase = std::f64::consts::TAU * frequency * index as f64 / f64::from(sample_rate);
            phase.sin() as f32
        })
        .collect();
    let spectrum = Spectrum::from_pcm(&samples, sample_rate, window);
    let peak = spectrum.peaks(1)[0].0;
    assert!((peak.0 - frequency).abs() <= f64::from(sample_rate) / window as f64);
}
