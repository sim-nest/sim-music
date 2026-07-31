use std::time::Duration;

use super::*;
use sim_lib_sound_core::{Amplitude, Frequency, Tone};

// conformance: sound PCM spectra delegate transform math and retain domain facts.

#[test]
fn spectrum_peaks_recover_sine_frequency() {
    let tone = Tone::sine(Frequency(440.0), Duration::from_secs(1));
    let spectrum = Spectrum::from_tone(&tone, Duration::from_millis(100));
    let peak = spectrum.peaks(1)[0].0;
    assert!((peak.0 - 440.0).abs() < 1e-6);
}

#[test]
fn pcm_spectrum_remains_a_sound_adapter_over_numbers_signal() {
    let samples = [1.0_f32, 0.0, -1.0, 0.0];
    let spectrum = Spectrum::from_pcm(&samples, 8, 4);
    assert_eq!(spectrum.bins.len(), 3);
    assert_eq!(spectrum.bins[0], (Frequency(0.0), Amplitude(0.0)));
    assert!((spectrum.bins[1].0.0 - 2.0).abs() < 1e-12);
    assert!((spectrum.bins[1].1.0 - 0.5).abs() < 1e-12);
    assert!(spectrum.bins[2].1.0 < 1e-12);
    assert_eq!(
        spectrum.source,
        SpectrumSource::FromPcm {
            window_size: 4,
            sample_rate: 8,
        }
    );
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

#[test]
fn stft_projection_retains_provenance_and_summary_apis() {
    let bins = [(0.0, 0.0), (0.25, -0.5), (0.0, 0.125)];
    let spectrum = Spectrum::from_stft_bins(&bins, 8_000, 4, -2).unwrap();
    assert_eq!(
        spectrum.source,
        SpectrumSource::FromStft {
            frame_size: 4,
            sample_rate: 8_000,
            onset_sample: -2,
        }
    );
    assert_eq!(spectrum.peaks(1)[0].0, Frequency(2_000.0));
    assert!(spectrum.centroid().0.is_finite());
    assert!(spectrum.flatness().is_finite());
    assert!(spectrum.rolloff(0.85).0.is_finite());
    assert_eq!(Spectrum::flux(&spectrum, &spectrum), 0.0);
}

#[test]
fn stft_projection_rejects_inconsistent_or_non_finite_frames() {
    assert_eq!(
        Spectrum::from_stft_bins(&[(0.0, 0.0)], 8_000, 4, 0),
        Err(SpectrumError::BinCount {
            expected: 3,
            actual: 1,
        })
    );
    assert_eq!(
        Spectrum::from_stft_bins(&[(0.0, 0.0), (f64::NAN, 0.0)], 8_000, 2, 0),
        Err(SpectrumError::NonFiniteBin { index: 1 })
    );
}
