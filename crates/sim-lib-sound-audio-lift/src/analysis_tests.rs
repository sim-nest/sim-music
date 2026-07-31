use sim_lib_numbers_signal::{
    Normalization, PaddingPolicy, SignConvention, WindowFunction, WindowSampling, WindowSpec,
};
use sim_lib_sound_tuning::EqualTemperament;

use crate::{
    AudioAnalysisControl, AudioAnalysisPlan, AudioFeatureSelection, BeatTrackingPlan,
    CepstralNormalization, CqtPlan, CqtWeighting, DctNormalization, Filterbank, FilterbankPlan,
    FrequencyScale, MfccPlan, OnsetPeak, OnsetPeaks, PeakPickingPlan, SpectralEnergy,
    StftOverlapPolicy, StftPlan, ZeroCrossingPlan, analyze_audio, track_beats, zero_crossing_rate,
};

// conformance: generated click/chord audio proves timing and labels end to end.

#[test]
fn generated_click_chord_fixture_proves_timing_features_and_labels() {
    let sample_rate = 8_000;
    let beat_samples = 4_000;
    let samples = click_chord_fixture(sample_rate, beat_samples);
    let plan = compact_analysis_plan(sample_rate);
    let control = AudioAnalysisControl {
        max_work: 100_000_000,
        max_results: 16,
        seed: 21,
    };
    let analysis = analyze_audio(
        &samples,
        sample_rate,
        &EqualTemperament::default(),
        &AudioFeatureSelection::foundry(),
        &plan,
        &control,
    )
    .unwrap();

    let onsets = analysis.onsets.as_ref().unwrap();
    assert!(onsets.peaks.len() >= 3, "{:#?}", onsets.peaks);
    for expected in [beat_samples, beat_samples * 2, beat_samples * 3] {
        assert!(
            onsets
                .peaks
                .iter()
                .any(|onset| onset.sample.abs_diff(expected as i64) <= 384),
            "missing onset near {expected}: {:#?}",
            onsets.peaks
        );
    }
    assert!(onsets.peaks.iter().all(|onset| onset.confidence > 0.0));
    assert!(onsets.latency_samples > 0);

    let beats = analysis.beats.as_ref().unwrap();
    assert!(
        beats
            .tempo_candidates
            .iter()
            .any(|candidate| { (candidate.bpm - 120.0).abs() < 2.0 && candidate.confidence > 0.0 })
    );
    assert!(beats.dynamic_programming.is_some());
    assert!(beats.beats.iter().all(|beat| beat.confidence > 0.0));
    assert!(
        beats
            .beats
            .iter()
            .skip(1)
            .all(|beat| { beat.bpm.is_some() && !beat.alternatives.is_empty() })
    );
    assert!(beats.meter_hypotheses.len() > 1);

    let mfcc = analysis.mfcc.as_ref().unwrap();
    assert_eq!(mfcc.sample_rate, sample_rate);
    assert_eq!(mfcc.plan.filterbank.scale, FrequencyScale::Mel);
    assert_eq!(mfcc.frames[0].coefficients.len(), 13);
    assert!(
        mfcc.frames
            .iter()
            .flat_map(|frame| &frame.coefficients)
            .all(|value| value.is_finite())
    );

    let keys = analysis.key.as_ref().unwrap();
    let chords = analysis.chords.as_ref().unwrap();
    assert!(keys.frames.iter().any(|frame| frame.label == "C major"));
    assert!(chords.frames.iter().any(|frame| frame.label == "C:maj"));
    assert!(chords.frames.iter().any(|frame| frame.label == "G:maj"));
    for frame in keys.frames.iter().chain(&chords.frames) {
        assert!(frame.confidence > 0.0);
        assert!(frame.alternatives.len() > 1);
        assert!(frame.alternatives.iter().all(|item| item.posterior > 0.0));
    }
    assert!(analysis.evidence.work_used <= analysis.evidence.work_limit);
    assert_eq!(analysis.evidence.seed, 21);
}

#[test]
fn beat_dp_preserves_changing_tempo_and_all_meter_alternatives() {
    let onsets = OnsetPeaks {
        plan: PeakPickingPlan::default(),
        latency_samples: 64,
        peaks: [0, 24_000, 44_000, 60_000]
            .into_iter()
            .enumerate()
            .map(|(frame_index, sample)| OnsetPeak {
                frame_index,
                sample,
                available_at_sample: sample + 64,
                strength: if frame_index.is_multiple_of(2) {
                    1.0
                } else {
                    0.7
                },
                confidence: 0.9,
            })
            .collect(),
        alternatives: Vec::new(),
        work_used: 1,
    };
    let tracked = track_beats(&onsets, 48_000, &BeatTrackingPlan::default()).unwrap();
    let tempi = tracked
        .beats
        .iter()
        .filter_map(|beat| beat.bpm)
        .collect::<Vec<_>>();
    assert_eq!(tempi.len(), 3);
    assert!(tempi.windows(2).any(|pair| (pair[0] - pair[1]).abs() > 1.0));
    assert!(tracked.dynamic_programming.is_some());
    assert!(tracked.meter_hypotheses.len() >= 4);
}

#[test]
fn zcr_and_each_perceptual_filterbank_retain_full_rate_policy() {
    let samples = (0_usize..800)
        .map(|index| if index.is_multiple_of(2) { -1.0 } else { 1.0 })
        .collect::<Vec<_>>();
    let zcr = zero_crossing_rate(
        &samples,
        8_000,
        &ZeroCrossingPlan {
            frame: 80,
            hop: 40,
            ..ZeroCrossingPlan::default()
        },
    )
    .unwrap();
    assert_eq!(zcr.sample_rate, 8_000);
    assert!(zcr.frames[0].rate > 0.99);

    for scale in [
        FrequencyScale::Mel,
        FrequencyScale::Bark,
        FrequencyScale::Erb,
    ] {
        let bank = Filterbank::new(
            48_000,
            1_024,
            &FilterbankPlan {
                scale,
                bands: 24,
                minimum_hz: 20.0,
                maximum_hz: Some(20_000.0),
                max_weights: 100_000,
            },
        )
        .unwrap();
        assert_eq!(bank.sample_rate, 48_000);
        assert_eq!(bank.bands.len(), 24);
        assert!(
            bank.bands
                .windows(2)
                .all(|pair| pair[0].center_hz < pair[1].center_hz)
        );
        assert!(
            bank.weights
                .iter()
                .all(|row| row.iter().all(|weight| (0.0..=1.0).contains(weight)))
        );
    }
}

fn compact_analysis_plan(sample_rate: u32) -> AudioAnalysisPlan {
    let mut window = WindowSpec::new(WindowFunction::Hann);
    window.sampling = WindowSampling::Periodic;
    AudioAnalysisPlan {
        stft: StftPlan {
            frame: 256,
            hop: 64,
            analysis_window: window.clone(),
            synthesis_window: window.clone(),
            center: true,
            padding: PaddingPolicy::Zero,
            phase: SignConvention::NegativeForward,
            normalization: Normalization::Forward,
            overlap: StftOverlapPolicy::RequireCola { tolerance: 1e-10 },
            max_frames: 2_048,
            max_cells: 1_000_000,
        },
        peak_picking: PeakPickingPlan {
            local_max_before: 2,
            local_max_after: 2,
            average_before: 8,
            average_after: 8,
            threshold: 0.04,
            minimum_distance_samples: 2_000,
            max_peaks: 16,
            max_work: 1_000_000,
        },
        zero_crossing: ZeroCrossingPlan {
            frame: 256,
            hop: 64,
            ..ZeroCrossingPlan::default()
        },
        mfcc: MfccPlan {
            filterbank: FilterbankPlan {
                scale: FrequencyScale::Mel,
                bands: 24,
                minimum_hz: 20.0,
                maximum_hz: Some(f64::from(sample_rate) / 2.0 - 1.0),
                max_weights: 100_000,
            },
            energy: SpectralEnergy::Power,
            log_floor: 1e-9,
            coefficients: 13,
            dct_normalization: DctNormalization::Orthonormal,
            lifter: Some(22.0),
            normalization: CepstralNormalization::MeanVariance {
                variance_floor: 1e-12,
            },
            max_work: 20_000_000,
        },
        constant_q: CqtPlan {
            hop: 256,
            min_frequency_hz: 110.0,
            max_frequency_hz: 1_760.0,
            bins_per_octave: 12,
            window,
            center: true,
            padding: PaddingPolicy::Zero,
            phase: SignConvention::NegativeForward,
            weighting: CqtWeighting::Power,
            max_window: 4_096,
            max_frames: 1_024,
            max_bins: 128,
            max_work: 50_000_000,
        },
        ..AudioAnalysisPlan::default()
    }
}

fn click_chord_fixture(sample_rate: u32, beat_samples: usize) -> Vec<f32> {
    let chords: [[f64; 3]; 4] = [
        [261.625_565, 329.627_557, 391.995_436],
        [261.625_565, 329.627_557, 391.995_436],
        [195.997_718, 246.941_651, 293.664_768],
        [195.997_718, 246.941_651, 293.664_768],
    ];
    (0..beat_samples * chords.len())
        .map(|index| {
            let beat = index / beat_samples;
            let within = index % beat_samples;
            let time = index as f64 / f64::from(sample_rate);
            let chord = chords[beat]
                .iter()
                .map(|frequency| (std::f64::consts::TAU * frequency * time).sin())
                .sum::<f64>()
                * 0.18;
            let click = if within < 24 {
                0.9 * (1.0 - within as f64 / 24.0)
                    * if within.is_multiple_of(2) { 1.0 } else { -1.0 }
            } else {
                0.0
            };
            (chord + click).clamp(-1.0, 1.0) as f32
        })
        .collect()
}
