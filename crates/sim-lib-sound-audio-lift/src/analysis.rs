//! Composed bounded PCM feature, rhythm, and harmonic analysis.

use sim_lib_music_analysis::{
    HarmonicDecodeError, HarmonicDecodePlan, HarmonicFeatureFrame, HarmonicSequence, decode_chords,
    decode_keys,
};
use sim_lib_sound_tuning::Tuning;
use thiserror::Error;

use crate::{
    AudioTransformError, BeatTracking, BeatTrackingPlan, Chroma, ChromaPlan, ConstantQ, CqtPlan,
    Mfcc, MfccPlan, OnsetPeaks, OnsetStrength, OnsetStrengthPlan, PeakPickingPlan, StftPlan,
    ZeroCrossingPlan, ZeroCrossingRate, chroma, constant_q, mfcc, onset_strength, pick_onsets,
    stft, track_beats, zero_crossing_rate,
};

/// Feature families selected for one composed PCM analysis.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioFeatureSelection {
    /// Return onset strength and selected onset peaks.
    pub onsets: bool,
    /// Return tempo candidates, varying-tempo beats, and meter hypotheses.
    pub beats: bool,
    /// Return framed zero-crossing rate.
    pub zero_crossing_rate: bool,
    /// Return mel/Bark/ERB MFCC vectors.
    pub mfcc: bool,
    /// Return tuning-anchored chroma.
    pub chroma: bool,
    /// Return key posterior sequence evidence.
    pub key: bool,
    /// Return chord HMM sequence evidence.
    pub chords: bool,
}

impl AudioFeatureSelection {
    /// Selection used by the foundry's complete analysis recipe.
    pub fn foundry() -> Self {
        Self {
            onsets: true,
            beats: true,
            zero_crossing_rate: true,
            mfcc: true,
            chroma: true,
            key: true,
            chords: true,
        }
    }

    fn needs_stft(&self) -> bool {
        self.onsets || self.beats || self.mfcc
    }

    fn needs_chroma(&self) -> bool {
        self.chroma || self.key || self.chords
    }

    fn any(&self) -> bool {
        self.needs_stft() || self.zero_crossing_rate || self.needs_chroma()
    }
}

/// Complete sub-algorithm policy for composed audio analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioAnalysisPlan {
    /// Framed Fourier policy shared by onset and MFCC analysis.
    pub stft: StftPlan,
    /// Onset novelty policy.
    pub onset_strength: OnsetStrengthPlan,
    /// Onset peak selection, latency, and distance policy.
    pub peak_picking: PeakPickingPlan,
    /// Tempo, dynamic-programming, and meter policy.
    pub beat_tracking: BeatTrackingPlan,
    /// Time-domain zero-crossing policy.
    pub zero_crossing: ZeroCrossingPlan,
    /// Perceptual filterbank, log, DCT, lifter, and normalization policy.
    pub mfcc: MfccPlan,
    /// Tuning-aligned constant-Q policy used before chroma.
    pub constant_q: CqtPlan,
    /// Chroma folding and normalization policy.
    pub chroma: ChromaPlan,
    /// Key posterior/HMM policy.
    pub key: HarmonicDecodePlan,
    /// Chord posterior/HMM policy.
    pub chords: HarmonicDecodePlan,
}

impl Default for AudioAnalysisPlan {
    fn default() -> Self {
        Self {
            stft: StftPlan::default(),
            onset_strength: OnsetStrengthPlan::default(),
            peak_picking: PeakPickingPlan::default(),
            beat_tracking: BeatTrackingPlan::default(),
            zero_crossing: ZeroCrossingPlan::default(),
            mfcc: MfccPlan::default(),
            constant_q: CqtPlan::default(),
            chroma: ChromaPlan::default(),
            key: HarmonicDecodePlan::default(),
            chords: HarmonicDecodePlan {
                strategy: sim_lib_music_analysis::HarmonicDecodeStrategy::Viterbi,
                stay_probability: 0.75,
                ..HarmonicDecodePlan::default()
            },
        }
    }
}

/// Cross-pipeline deterministic bounds retained in every analysis result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioAnalysisControl {
    /// Maximum aggregate charged work across selected algorithms.
    pub max_work: u64,
    /// Maximum onset peaks and per-frame inference alternatives.
    pub max_results: usize,
    /// Declared deterministic seed; current algorithms consume no randomness.
    pub seed: u64,
}

impl Default for AudioAnalysisControl {
    fn default() -> Self {
        Self {
            max_work: 1_000_000_000,
            max_results: 16,
            seed: 0,
        }
    }
}

/// Aggregate admission and completion evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioAnalysisEvidence {
    /// Sum of selected sub-algorithm work receipts.
    pub work_used: u64,
    /// Caller-declared aggregate work ceiling.
    pub work_limit: u64,
    /// Caller-declared result ceiling.
    pub result_limit: usize,
    /// Caller-declared deterministic seed.
    pub seed: u64,
}

/// Composed PCM analysis with each unrequested family omitted.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioAnalysis {
    /// Full source sample rate.
    pub sample_rate: u32,
    /// Selected feature families.
    pub selection: AudioFeatureSelection,
    /// Onset novelty curve when required by onsets or beats.
    pub onset_strength: Option<OnsetStrength>,
    /// Selected onset peaks and rejected alternatives.
    pub onsets: Option<OnsetPeaks>,
    /// Tempo, beat, meter, and staged-DP evidence.
    pub beats: Option<BeatTracking>,
    /// Framed zero-crossing rate.
    pub zero_crossing_rate: Option<ZeroCrossingRate>,
    /// Policy-complete MFCC output.
    pub mfcc: Option<Mfcc>,
    /// Tuning-aligned constant-Q intermediate retained with chroma requests.
    pub constant_q: Option<ConstantQ>,
    /// Octave-folded chroma.
    pub chroma: Option<Chroma>,
    /// Key sequence with posterior alternatives.
    pub key: Option<HarmonicSequence>,
    /// Chord sequence with posterior alternatives.
    pub chords: Option<HarmonicSequence>,
    /// Aggregate deterministic evidence.
    pub evidence: AudioAnalysisEvidence,
}

/// Failure from composed feature, graph, or HMM analysis.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AudioAnalysisError {
    /// No feature family was selected or a global bound was zero.
    #[error("invalid audio analysis request: {0}")]
    Invalid(&'static str),
    /// Aggregate completed work exceeded the caller's bound.
    #[error("audio analysis used {used} work units, exceeding {maximum}")]
    WorkLimit {
        /// Completed work charged by selected sub-algorithms.
        used: u64,
        /// Caller-declared aggregate ceiling.
        maximum: u64,
    },
    /// Framing, spectral, onset, or beat analysis failed.
    #[error(transparent)]
    Transform(#[from] AudioTransformError),
    /// Key or chord HMM adaptation failed.
    #[error(transparent)]
    Harmonic(#[from] HarmonicDecodeError),
}

/// Runs the selected bounded analysis families over one PCM buffer.
pub fn analyze_audio(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    selection: &AudioFeatureSelection,
    plan: &AudioAnalysisPlan,
    control: &AudioAnalysisControl,
) -> Result<AudioAnalysis, AudioAnalysisError> {
    if !selection.any() {
        return Err(AudioAnalysisError::Invalid(
            "at least one feature family must be selected",
        ));
    }
    if control.max_work == 0 || control.max_results == 0 {
        return Err(AudioAnalysisError::Invalid(
            "work and result bounds must be positive",
        ));
    }
    let mut work_used = 0_u64;
    let transformed = selection
        .needs_stft()
        .then(|| stft(samples, sample_rate, &plan.stft))
        .transpose()?;
    if let Some(transformed) = &transformed {
        add_work(
            &mut work_used,
            transformed
                .frames
                .len()
                .saturating_mul(plan.stft.frame / 2 + 1) as u64,
            control,
        )?;
    }

    let strength = transformed
        .as_ref()
        .filter(|_| selection.onsets || selection.beats)
        .map(|transformed| onset_strength(transformed, &plan.onset_strength))
        .transpose()?;
    if let Some(strength) = &strength {
        add_work(&mut work_used, strength.work_used, control)?;
    }
    let mut peak_plan = plan.peak_picking.clone();
    peak_plan.max_peaks = peak_plan.max_peaks.min(control.max_results);
    let onset_peaks = strength
        .as_ref()
        .map(|strength| pick_onsets(strength, plan.stft.hop, &peak_plan))
        .transpose()?;
    if let Some(onsets) = &onset_peaks {
        add_work(&mut work_used, onsets.work_used, control)?;
    }
    let beats = if selection.beats {
        let onsets = onset_peaks
            .as_ref()
            .expect("beat selection constructs onset peaks");
        let beats = track_beats(onsets, sample_rate, &plan.beat_tracking)?;
        if let Some(dp) = &beats.dynamic_programming {
            add_work(&mut work_used, dp.receipt.work_used, control)?;
        }
        Some(beats)
    } else {
        None
    };

    let zero_crossing = selection
        .zero_crossing_rate
        .then(|| zero_crossing_rate(samples, sample_rate, &plan.zero_crossing))
        .transpose()?;
    if let Some(zero_crossing) = &zero_crossing {
        add_work(&mut work_used, zero_crossing.work_used, control)?;
    }
    let cepstra = if selection.mfcc {
        let cepstra = mfcc(
            transformed
                .as_ref()
                .expect("MFCC selection constructs an STFT"),
            &plan.mfcc,
        )?;
        add_work(&mut work_used, cepstra.work_used, control)?;
        Some(cepstra)
    } else {
        None
    };

    let cqt = selection
        .needs_chroma()
        .then(|| constant_q(samples, sample_rate, tuning, &plan.constant_q))
        .transpose()?;
    if let Some(cqt) = &cqt {
        add_work(&mut work_used, cqt.report.work_units, control)?;
    }
    let folded = cqt
        .as_ref()
        .map(|cqt| chroma(cqt, &plan.chroma))
        .transpose()?;
    let harmonic_features = folded.as_ref().map(harmonic_feature_frames);
    let mut key_plan = plan.key.clone();
    key_plan.max_alternatives = key_plan.max_alternatives.min(control.max_results);
    key_plan.max_work = key_plan
        .max_work
        .min(control.max_work.saturating_sub(work_used));
    let key = selection
        .key
        .then(|| decode_keys(harmonic_features.as_deref().unwrap_or_default(), &key_plan))
        .transpose()?;
    if let Some(key) = &key {
        add_work(&mut work_used, key.evidence.work_used, control)?;
    }
    let mut chord_plan = plan.chords.clone();
    chord_plan.max_alternatives = chord_plan.max_alternatives.min(control.max_results);
    chord_plan.max_work = chord_plan
        .max_work
        .min(control.max_work.saturating_sub(work_used));
    let chords = selection
        .chords
        .then(|| {
            decode_chords(
                harmonic_features.as_deref().unwrap_or_default(),
                &chord_plan,
            )
        })
        .transpose()?;
    if let Some(chords) = &chords {
        add_work(&mut work_used, chords.evidence.work_used, control)?;
    }
    Ok(AudioAnalysis {
        sample_rate,
        selection: selection.clone(),
        onset_strength: strength,
        onsets: onset_peaks,
        beats,
        zero_crossing_rate: zero_crossing,
        mfcc: cepstra,
        constant_q: cqt,
        chroma: folded,
        key,
        chords,
        evidence: AudioAnalysisEvidence {
            work_used,
            work_limit: control.max_work,
            result_limit: control.max_results,
            seed: control.seed,
        },
    })
}

fn harmonic_feature_frames(chroma: &Chroma) -> Vec<HarmonicFeatureFrame> {
    chroma
        .frames
        .iter()
        .map(|frame| {
            let mut values = frame.bins.clone();
            if values.iter().all(|value| *value <= f64::EPSILON) {
                values.fill(1.0);
            }
            HarmonicFeatureFrame {
                at_sample: frame.onset_sample,
                values,
            }
        })
        .collect()
}

fn add_work(
    used: &mut u64,
    amount: u64,
    control: &AudioAnalysisControl,
) -> Result<(), AudioAnalysisError> {
    *used = used.checked_add(amount).unwrap_or(u64::MAX);
    if *used > control.max_work {
        return Err(AudioAnalysisError::WorkLimit {
            used: *used,
            maximum: control.max_work,
        });
    }
    Ok(())
}
