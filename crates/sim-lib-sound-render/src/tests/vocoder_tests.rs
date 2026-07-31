use sim_lib_numbers_signal::{
    Normalization, PaddingPolicy, SignConvention, WindowFunction, WindowSampling, WindowSpec,
};
use sim_lib_sound_audio_lift::{StftOverlapPolicy, StftPlan};

use crate::{
    InstantaneousFrequencyPolicy, PhaseLockPolicy, PhaseUnwrapPolicy, TransientPolicy,
    VocoderPolicy, phase_vocode,
};

#[test]
fn phase_vocoder_preserves_duration_and_pitch_independently() {
    let input = sine(440.0, 8_000, 8_000);
    let stretched = phase_vocode(
        &input,
        VocoderPolicy {
            stretch_ratio: 1.5,
            ..policy(8_000)
        },
    )
    .unwrap();
    assert_eq!(stretched.samples.len(), 12_000);
    assert!((dominant_frequency(&stretched.samples, 8_000) - 440.0).abs() < 18.0);

    let shifted = phase_vocode(
        &input,
        VocoderPolicy {
            pitch_ratio: 2.0,
            ..policy(8_000)
        },
    )
    .unwrap();
    assert_eq!(shifted.samples.len(), input.len());
    let shifted_frequency = dominant_frequency(&shifted.samples, 8_000);
    assert!(
        (shifted_frequency - 880.0).abs() < 20.0,
        "shifted dominant frequency was {shifted_frequency} Hz"
    );
    assert_eq!(shifted.policy.pitch_ratio, 2.0);
    assert_eq!(shifted.clipped_samples, 0);
}

#[test]
fn explicit_phase_and_transient_policies_are_retained_and_exercised() {
    let mut input = vec![0.0; 4_096];
    for (index, sample) in input.iter_mut().enumerate().skip(2_048) {
        *sample = (std::f64::consts::TAU * 523.25 * index as f64 / 8_000.0).sin() as f32;
    }
    let policy = VocoderPolicy {
        stretch_ratio: 1.25,
        phase_lock: PhaseLockPolicy::Identity { radius_bins: 4 },
        transient: TransientPolicy::Reset {
            spectral_flux_threshold: 0.1,
        },
        phase_unwrap: PhaseUnwrapPolicy::ExpectedAdvance {
            discontinuity_radians: std::f64::consts::PI,
        },
        instantaneous_frequency: InstantaneousFrequencyPolicy::PhaseDerivative,
        ..policy(8_000)
    };
    let report = phase_vocode(&input, policy.clone()).unwrap();

    assert_eq!(report.policy, policy);
    assert!(report.transient_resets > 0);
    assert!(report.analysis_frames > 10);
    assert!(report.samples.iter().all(|sample| sample.is_finite()));
    assert!(report.peak > 0.1);
}

#[test]
fn bin_center_policy_remains_a_deliberate_lower_fidelity_option() {
    let report = phase_vocode(
        &sine(330.0, 8_000, 2_000),
        VocoderPolicy {
            phase_lock: PhaseLockPolicy::Independent,
            transient: TransientPolicy::Smear,
            instantaneous_frequency: InstantaneousFrequencyPolicy::BinCenter,
            ..policy(8_000)
        },
    )
    .unwrap();
    assert_eq!(report.samples.len(), 2_000);
    assert!(report.samples.iter().all(|sample| sample.is_finite()));
}

fn policy(sample_rate_hz: u32) -> VocoderPolicy {
    let mut window = WindowSpec::new(WindowFunction::Hann);
    window.sampling = WindowSampling::Periodic;
    VocoderPolicy {
        sample_rate_hz,
        stft: StftPlan {
            frame: 256,
            hop: 64,
            analysis_window: window.clone(),
            synthesis_window: window,
            center: true,
            padding: PaddingPolicy::Zero,
            phase: SignConvention::NegativeForward,
            normalization: Normalization::Forward,
            overlap: StftOverlapPolicy::RequireCola { tolerance: 1e-10 },
            max_frames: 1_024,
            max_cells: 1_048_576,
        },
        max_output_samples: 32_000,
        ..VocoderPolicy::default()
    }
}

fn sine(frequency_hz: f64, sample_rate_hz: u32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|index| {
            (std::f64::consts::TAU * frequency_hz * index as f64 / f64::from(sample_rate_hz)).sin()
                as f32
                * 0.5
        })
        .collect()
}

fn dominant_frequency(samples: &[f32], sample_rate_hz: u32) -> f64 {
    let start = samples.len() / 4;
    let end = samples.len() * 3 / 4;
    let window = &samples[start..end];
    (100..1_500)
        .step_by(2)
        .map(|frequency| {
            let magnitude = window
                .iter()
                .enumerate()
                .map(|(index, sample)| {
                    let angle = std::f64::consts::TAU * frequency as f64 * index as f64
                        / f64::from(sample_rate_hz);
                    (
                        f64::from(*sample) * angle.cos(),
                        f64::from(*sample) * angle.sin(),
                    )
                })
                .fold((0.0, 0.0), |(real, imaginary), value| {
                    (real + value.0, imaginary + value.1)
                });
            (frequency, magnitude.0.hypot(magnitude.1))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(frequency, _)| frequency as f64)
        .unwrap()
}
