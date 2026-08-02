use sim_lib_numbers_signal::{
    Normalization, PaddingPolicy, SignConvention, WindowFunction, WindowSampling, WindowSpec,
};
use sim_lib_sound_spectrum::SpectrumSource;
use sim_lib_sound_tuning::EqualTemperament;

use crate::{
    AudioTransformError, ChromaFoldPolicy, ChromaNormalization, ChromaPlan, CqtPlan, CqtWeighting,
    StftOverlapPolicy, StftPlan, chroma, cola_report, constant_q, istft, stft,
};

// conformance: audio transforms retain policies, bounded work, tuning, and folding evidence.

#[test]
fn stft_round_trips_when_declared_window_pair_is_cola() {
    let samples = (0..997)
        .map(|index| {
            let time = index as f64 / 8_000.0;
            ((std::f64::consts::TAU * 440.0 * time).sin()
                + 0.2 * (std::f64::consts::TAU * 713.0 * time).cos()) as f32
        })
        .collect::<Vec<_>>();
    let plan = stft_plan(128, 32);
    let transformed = stft(&samples, 8_000, &plan).unwrap();

    assert!(transformed.cola.reconstructable);
    assert!((transformed.cola.gain_min - 1.5).abs() < 1e-12);
    assert_eq!(transformed.plan.padding, PaddingPolicy::Zero);
    assert_eq!(transformed.plan.phase, SignConvention::NegativeForward);
    assert_eq!(transformed.plan.normalization, Normalization::Forward);
    assert!(transformed.plan.center);

    let recovered = istft(&transformed).unwrap();
    assert_eq!(recovered.len(), samples.len());
    let max_error = samples
        .iter()
        .zip(recovered)
        .map(|(expected, actual)| (expected - actual).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_error < 2e-5, "maximum reconstruction error {max_error}");
}

#[test]
fn invalid_overlap_pair_returns_its_failing_cola_report() {
    let plan = stft_plan(128, 40);
    let report = cola_report(&plan).unwrap();
    assert!(!report.reconstructable);
    assert!(report.gain_max - report.gain_min > report.tolerance);

    let error = stft(&[0.0; 256], 8_000, &plan).unwrap_err();
    let AudioTransformError::NonCola { report: failed } = error else {
        panic!("expected failing COLA evidence");
    };
    assert_eq!(failed, report);
}

#[test]
fn stft_frames_feed_existing_sound_spectrum_summaries() {
    let samples = sine_mix(&[(437.5, 1.0)], 8_000, 1_024);
    let transformed = stft(&samples, 8_000, &stft_plan(512, 128)).unwrap();
    let frame = transformed
        .frames
        .iter()
        .find(|frame| frame.onset_sample == 0)
        .expect("unpadded frame");
    let spectrum = frame.spectrum(8_000, 512).unwrap();

    assert_eq!(
        spectrum.source,
        SpectrumSource::FromStft {
            frame_size: 512,
            sample_rate: 8_000,
            onset_sample: 0,
        }
    );
    assert!((spectrum.peaks(1)[0].0.0 - 437.5).abs() < 1e-9);
    assert!(spectrum.centroid().0.is_finite());
    assert!(spectrum.flatness().is_finite());
    assert!(spectrum.rolloff(0.85).0.is_finite());
    assert_eq!(
        sim_lib_sound_spectrum::Spectrum::flux(&spectrum, &spectrum),
        0.0
    );
}

#[test]
fn bounded_cqt_and_chroma_name_tuning_weighting_and_fold_policy() {
    let samples = sine_mix(&[(440.0, 1.0), (659.255_113_8, 0.5)], 8_000, 4_096);
    let tuning = EqualTemperament::default();
    let plan = CqtPlan {
        hop: 256,
        min_frequency_hz: 220.0,
        max_frequency_hz: 880.0,
        max_window: 4_096,
        max_work: 20_000_000,
        weighting: CqtWeighting::Power,
        ..CqtPlan::default()
    };
    let cqt = constant_q(&samples, 8_000, &tuning, &plan).unwrap();

    assert_eq!(cqt.reference.tuning, "equal-temperament");
    assert_eq!(cqt.reference.frequency.0, 440.0);
    assert_eq!(cqt.reference.divisions, 12);
    assert_eq!(cqt.report.bins, 25);
    assert!(cqt.report.work_units <= cqt.report.work_limit);
    let middle = &cqt.frames[cqt.frames.len() / 2];
    let strongest = middle
        .bins
        .iter()
        .max_by(|left, right| left.value.total_cmp(&right.value))
        .unwrap();
    assert!((strongest.center_frequency.0 - 440.0).abs() < 1e-9);

    let chroma_plan = ChromaPlan {
        folding: ChromaFoldPolicy::Sum,
        normalization: ChromaNormalization::L1,
    };
    let folded = chroma(&cqt, &chroma_plan).unwrap();
    assert_eq!(folded.reference.tuning, "equal-temperament");
    assert_eq!(folded.weighting, CqtWeighting::Power);
    assert_eq!(folded.plan.folding, ChromaFoldPolicy::Sum);
    let middle = &folded.frames[folded.frames.len() / 2];
    assert_eq!(middle.bins.len(), 12);
    assert_eq!(
        middle
            .bins
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .map(|(index, _)| index),
        Some(9),
        "A chroma must dominate: {:?}",
        middle.bins
    );
    assert!((middle.bins.iter().sum::<f64>() - 1.0).abs() < 1e-12);
}

#[test]
fn cqt_fails_closed_before_exceeding_declared_work() {
    let samples = sine_mix(&[(440.0, 1.0)], 8_000, 2_048);
    let plan = CqtPlan {
        min_frequency_hz: 220.0,
        max_frequency_hz: 880.0,
        max_window: 4_096,
        max_work: 1,
        ..CqtPlan::default()
    };
    let error = constant_q(&samples, 8_000, &EqualTemperament::default(), &plan).unwrap_err();
    assert!(matches!(error, AudioTransformError::WorkLimit { .. }));
}

fn stft_plan(frame: usize, hop: usize) -> StftPlan {
    let mut window = WindowSpec::new(WindowFunction::Hann);
    window.sampling = WindowSampling::Periodic;
    StftPlan {
        frame,
        hop,
        analysis_window: window.clone(),
        synthesis_window: window,
        center: true,
        padding: PaddingPolicy::Zero,
        phase: SignConvention::NegativeForward,
        normalization: Normalization::Forward,
        overlap: StftOverlapPolicy::RequireCola { tolerance: 1e-10 },
        max_frames: 4_096,
        max_cells: 4_194_304,
    }
}

fn sine_mix(tones: &[(f64, f64)], sample_rate: u32, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|index| {
            let time = index as f64 / f64::from(sample_rate);
            tones
                .iter()
                .map(|(frequency, amplitude)| {
                    amplitude * (std::f64::consts::TAU * frequency * time).sin()
                })
                .sum::<f64>() as f32
        })
        .collect()
}
