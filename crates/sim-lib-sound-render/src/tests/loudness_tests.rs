use crate::{
    FrequencyWeighting, GatingPolicy, LoudnessLayout, LoudnessSpec, NormalizationSpec,
    TruePeakPolicy, measure_loudness, normalize_loudness,
};

#[test]
fn ebu_reference_sine_measures_integrated_momentary_and_true_peak() {
    let samples = stereo_reference_sine(3.0);
    let report = measure_loudness(&samples, spec()).unwrap();
    let integrated = report.integrated_lufs.unwrap();

    assert!(
        (integrated - -23.0).abs() < 0.15,
        "integrated {integrated} LUFS"
    );
    assert_eq!(report.momentary.len(), 27);
    assert!(
        report
            .momentary
            .iter()
            .all(|block| (block.lufs.unwrap() - -23.0).abs() < 0.2)
    );
    assert_eq!(report.absolute_gate_lufs, Some(-70.0));
    assert_eq!(report.gated_blocks, report.momentary.len());
    assert!(report.true_peak.true_peak >= report.true_peak.sample_peak);
    assert!((report.true_peak.true_peak_dbtp.unwrap() - -23.0).abs() < 0.2);
}

#[test]
fn ebu_relative_gate_excludes_silence_without_hiding_gate_evidence() {
    let mut samples = stereo_reference_sine(2.0);
    samples.extend(vec![0.0; 48_000 * 2 * 2]);
    let report = measure_loudness(&samples, spec()).unwrap();

    assert!(report.gated_blocks < report.momentary.len());
    assert!(report.relative_gate_lufs.is_some());
    assert!((report.integrated_lufs.unwrap() - -23.0).abs() < 0.5);
    assert!(report.momentary.iter().any(|block| block.lufs.is_none()));
}

#[test]
fn true_peak_interpolation_detects_an_intersample_overshoot() {
    let pattern = [0.9, 0.9, -0.9, -0.9];
    let samples = pattern.into_iter().cycle().take(48_000).collect::<Vec<_>>();
    let report = measure_loudness(
        &samples,
        LoudnessSpec {
            layout: LoudnessLayout::mono(),
            frequency_weighting: FrequencyWeighting::Flat,
            gating: GatingPolicy::None,
            ..spec()
        },
    )
    .unwrap();
    assert_eq!(report.true_peak.sample_peak, 0.9f32 as f64);
    assert!(report.true_peak.true_peak > report.true_peak.sample_peak + 0.05);
}

#[test]
fn normalization_reports_gain_ceiling_and_unclipped_float_output() {
    let samples = stereo_reference_sine(2.0);
    let report = normalize_loudness(
        &samples,
        spec(),
        NormalizationSpec {
            target_lufs: -16.0,
            max_true_peak_dbtp: -18.0,
            max_abs_gain_db: 12.0,
        },
    )
    .unwrap();

    assert!((report.requested_gain_db - 7.0).abs() < 0.15);
    assert_eq!(report.applied_gain_db, report.requested_gain_db);
    assert!(!report.gain_limited);
    assert!((report.output.integrated_lufs.unwrap() - -16.0).abs() < 0.05);
    assert!(report.true_peak_ceiling_exceeded);
    assert_eq!(report.clipped_samples, 0);
}

fn spec() -> LoudnessSpec {
    LoudnessSpec {
        sample_rate_hz: 48_000,
        layout: LoudnessLayout::stereo(),
        frequency_weighting: FrequencyWeighting::ItuRBs1770K,
        gating: GatingPolicy::EbuR128,
        true_peak: TruePeakPolicy {
            max_work: 100_000_000,
            ..TruePeakPolicy::default()
        },
        max_frames: 48_000 * 8,
    }
}

fn stereo_reference_sine(seconds: f64) -> Vec<f32> {
    let frames = (48_000.0 * seconds) as usize;
    let peak = 2.0f64.sqrt() * 10.0f64.powf(-23.0 / 20.0) / 2.0f64.sqrt();
    let mut samples = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let sample = (std::f64::consts::TAU * 1_000.0 * frame as f64 / 48_000.0).sin() * peak;
        samples.extend([sample as f32, sample as f32]);
    }
    samples
}
