//! Monophonic YIN and probabilistic-YIN pitch tracking.

use sim_lib_pitch_core::Pitch;
use sim_lib_sound_core::Frequency;
use sim_lib_sound_tuning::Tuning;

use crate::{AudioLiftError, AudioLiftReport};

/// Time-domain pitch estimator used for each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitchTrackMethod {
    /// Select the first cumulative-mean normalized-difference trough below one threshold.
    Yin,
    /// Rank troughs by the mass of an explicit threshold distribution.
    Pyin,
}

/// Sub-sample lag refinement applied after YIN trough selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitchInterpolation {
    /// Retain the selected integer lag without refinement.
    None,
    /// Fit a parabola through the trough and its immediate neighbors.
    Parabolic,
}

/// Treatment of a final PCM fragment shorter than the declared frame size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitchFrameTail {
    /// Do not analyze an incomplete final frame.
    Drop,
    /// Extend an incomplete final frame with zeros and retain the original duration.
    ZeroPad,
}

/// Framing policy retained with every pitch estimate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchFramePolicy {
    /// Samples in one analysis frame.
    pub size: usize,
    /// Samples advanced between frame onsets.
    pub hop: usize,
    /// Final-fragment policy.
    pub tail: PitchFrameTail,
}

impl Default for PitchFramePolicy {
    fn default() -> Self {
        Self {
            size: 2_048,
            hop: 256,
            tail: PitchFrameTail::ZeroPad,
        }
    }
}

/// Inclusive frequency range searched by a pitch estimator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PitchRange {
    /// Lowest admitted fundamental in hertz.
    pub min_hz: f64,
    /// Highest admitted fundamental in hertz.
    pub max_hz: f64,
}

impl PitchRange {
    /// Builds a validated range.
    pub fn new(min_hz: f64, max_hz: f64) -> Result<Self, AudioLiftError> {
        let range = Self { min_hz, max_hz };
        range.validate()?;
        Ok(range)
    }

    fn validate(self) -> Result<(), AudioLiftError> {
        if !self.min_hz.is_finite()
            || !self.max_hz.is_finite()
            || self.min_hz <= 0.0
            || self.max_hz <= self.min_hz
        {
            return Err(AudioLiftError::InvalidPitchRange);
        }
        Ok(())
    }
}

impl Default for PitchRange {
    fn default() -> Self {
        Self {
            min_hz: 55.0,
            max_hz: 1_760.0,
        }
    }
}

/// YIN trough selection and voicing policy.
#[derive(Clone, Debug, PartialEq)]
pub struct YinPolicy {
    /// Absolute threshold used by deterministic YIN.
    pub threshold: f64,
    /// Ordered thresholds whose uniform mass defines pYIN probability.
    pub pyin_thresholds: Vec<f64>,
    /// Minimum probability required to accept a voiced hypothesis.
    pub min_voiced_probability: f64,
    /// Root-mean-square amplitude at or below which a frame is silent.
    pub silence_rms: f64,
}

impl Default for YinPolicy {
    fn default() -> Self {
        Self {
            threshold: 0.15,
            pyin_thresholds: vec![0.05, 0.075, 0.10, 0.125, 0.15, 0.20, 0.25, 0.30],
            min_voiced_probability: 0.20,
            silence_rms: 1e-4,
        }
    }
}

/// Deterministic resource and result limits for pitch tracking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchTrackControl {
    /// Maximum charged difference-function sample comparisons.
    pub max_work: u64,
    /// Maximum retained hypotheses per frame.
    pub max_results: usize,
    /// Reproducibility seed retained in the report; the algorithm itself is deterministic.
    pub seed: u64,
}

impl Default for PitchTrackControl {
    fn default() -> Self {
        Self {
            max_work: 500_000,
            max_results: 8,
            seed: 0,
        }
    }
}

/// Complete monophonic pitch-tracking request.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchTrackPlan {
    /// Frame estimator.
    pub method: PitchTrackMethod,
    /// Admitted frequency range.
    pub range: PitchRange,
    /// Framing policy.
    pub frames: PitchFramePolicy,
    /// Lag-refinement policy.
    pub interpolation: PitchInterpolation,
    /// YIN/pYIN thresholds.
    pub yin: YinPolicy,
    /// Resource and result bounds.
    pub control: PitchTrackControl,
}

impl Default for PitchTrackPlan {
    fn default() -> Self {
        Self {
            method: PitchTrackMethod::Pyin,
            range: PitchRange::default(),
            frames: PitchFramePolicy::default(),
            interpolation: PitchInterpolation::Parabolic,
            yin: YinPolicy::default(),
            control: PitchTrackControl::default(),
        }
    }
}

impl PitchTrackPlan {
    /// Validates range, framing, thresholds, and deterministic bounds.
    pub fn validate(&self) -> Result<(), AudioLiftError> {
        self.range.validate()?;
        if self.frames.size < 8
            || self.frames.hop == 0
            || self.control.max_work == 0
            || self.control.max_results == 0
        {
            return Err(AudioLiftError::InvalidPitchBound);
        }
        let probabilities = self
            .yin
            .pyin_thresholds
            .iter()
            .copied()
            .chain([self.yin.threshold, self.yin.min_voiced_probability]);
        if self.yin.pyin_thresholds.is_empty()
            || probabilities
                .into_iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || !self.yin.silence_rms.is_finite()
            || self.yin.silence_rms < 0.0
        {
            return Err(AudioLiftError::InvalidPitchThreshold);
        }
        Ok(())
    }
}

/// Exact PCM location and frame policy that produced one estimate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PitchFrameProvenance {
    /// Zero-based analysis-frame index.
    pub frame_index: usize,
    /// Onset in source PCM samples.
    pub onset_sample: usize,
    /// Number of source samples present before padding.
    pub source_samples: usize,
    /// Declared analysis size.
    pub frame_size: usize,
    /// Declared analysis hop.
    pub hop_size: usize,
    /// Source sample rate.
    pub sample_rate: u32,
    /// Whether zeros were appended to fill the frame.
    pub zero_padded: bool,
}

/// Why a computed pitch hypothesis was not retained as voiced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitchRejectionReason {
    /// Frame energy did not exceed the silence policy.
    Silence,
    /// No admissible local trough passed deterministic YIN's threshold.
    Threshold,
    /// pYIN threshold mass did not reach the voiced-probability floor.
    VoicedProbability,
    /// A stronger hypothesis displaced this one at the result bound.
    ResultLimit,
}

/// One accepted or rejected lag hypothesis with uncertainty.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchHypothesis {
    /// Nearest pitch under the supplied tuning.
    pub pitch: Pitch,
    /// Parabolically interpolated fundamental estimate.
    pub frequency: Frequency,
    /// Conservative lower frequency bound from the adjacent lag cell.
    pub lower_frequency: Frequency,
    /// Conservative upper frequency bound from the adjacent lag cell.
    pub upper_frequency: Frequency,
    /// Integer difference-function lag.
    pub lag: usize,
    /// Parabolically interpolated lag.
    pub interpolated_lag: f64,
    /// Periodicity `1 - cumulative_mean_normalized_difference`.
    pub periodicity: f64,
    /// Probability assigned by the selected YIN policy.
    pub voiced_probability: f64,
    /// Confidence after periodicity and energy weighting.
    pub confidence: f64,
    /// Difference from the nearest tuned pitch in cents.
    pub cents_error: f64,
}

/// A rejected hypothesis and its explicit reason.
#[derive(Clone, Debug, PartialEq)]
pub struct RejectedPitchHypothesis {
    /// Computed hypothesis, absent only for an energy-classified silent frame.
    pub hypothesis: Option<PitchHypothesis>,
    /// Rejection policy that applied.
    pub reason: PitchRejectionReason,
}

/// Pitch evidence for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchTrackFrame {
    /// Exact frame provenance.
    pub provenance: PitchFrameProvenance,
    /// Retained candidates, strongest first.
    pub candidates: Vec<PitchHypothesis>,
    /// Hypotheses evaluated but rejected.
    pub rejected: Vec<RejectedPitchHypothesis>,
}

/// Monophonic track plus complete frame evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct PitchTrack {
    /// Complete validated request, retained for reconstruction and review.
    pub plan: PitchTrackPlan,
    /// Charged deterministic work.
    pub work_used: u64,
    /// Per-frame candidates and rejected hypotheses.
    pub frames: Vec<PitchTrackFrame>,
    /// Strongest voiced candidate in each frame, or `None` for unvoiced frames.
    pub contour: Vec<Option<PitchHypothesis>>,
}

/// Tracks a monophonic fundamental with YIN or probabilistic YIN.
pub fn pitch_track(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    plan: &PitchTrackPlan,
) -> Result<AudioLiftReport<PitchTrack>, AudioLiftError> {
    if sample_rate == 0 {
        return Err(AudioLiftError::InvalidSampleRate);
    }
    plan.validate()?;
    let mut work = WorkMeter::new(plan.control.max_work);
    let mut frames = Vec::new();
    let mut onset = 0usize;
    while onset < samples.len() {
        let remaining = samples.len() - onset;
        if remaining < plan.frames.size && plan.frames.tail == PitchFrameTail::Drop {
            break;
        }
        let source_samples = remaining.min(plan.frames.size);
        let mut owned = Vec::new();
        let frame = if source_samples == plan.frames.size {
            &samples[onset..onset + plan.frames.size]
        } else {
            owned.extend_from_slice(&samples[onset..]);
            owned.resize(plan.frames.size, 0.0);
            &owned
        };
        let provenance = PitchFrameProvenance {
            frame_index: frames.len(),
            onset_sample: onset,
            source_samples,
            frame_size: plan.frames.size,
            hop_size: plan.frames.hop,
            sample_rate,
            zero_padded: source_samples < plan.frames.size,
        };
        frames.push(analyze_frame(
            frame,
            sample_rate,
            tuning,
            plan,
            provenance,
            &mut work,
        )?);
        if source_samples < plan.frames.size {
            break;
        }
        onset = onset.saturating_add(plan.frames.hop);
    }
    let contour = frames
        .iter()
        .map(|frame| frame.candidates.first().cloned())
        .collect();
    Ok(AudioLiftReport {
        value: PitchTrack {
            plan: plan.clone(),
            work_used: work.used,
            frames,
            contour,
        },
        diagnostics: Vec::new(),
    })
}

fn analyze_frame(
    samples: &[f32],
    sample_rate: u32,
    tuning: &dyn Tuning,
    plan: &PitchTrackPlan,
    provenance: PitchFrameProvenance,
    work: &mut WorkMeter,
) -> Result<PitchTrackFrame, AudioLiftError> {
    let rms = (samples
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt();
    if rms <= plan.yin.silence_rms {
        return Ok(PitchTrackFrame {
            provenance,
            candidates: Vec::new(),
            rejected: vec![RejectedPitchHypothesis {
                hypothesis: None,
                reason: PitchRejectionReason::Silence,
            }],
        });
    }

    let min_lag = ((f64::from(sample_rate) / plan.range.max_hz).floor() as usize).max(2);
    let max_lag = ((f64::from(sample_rate) / plan.range.min_hz).ceil() as usize)
        .min(samples.len().saturating_sub(2));
    if min_lag > max_lag {
        return Err(AudioLiftError::InvalidPitchRange);
    }
    let mut difference = vec![0.0; max_lag + 1];
    for lag in 1..=max_lag {
        let pairs = samples.len() - lag;
        work.charge(pairs as u64)?;
        difference[lag] = samples[..pairs]
            .iter()
            .zip(&samples[lag..])
            .map(|(left, right)| {
                let delta = f64::from(*left) - f64::from(*right);
                delta * delta
            })
            .sum();
    }
    let mut cmnd = vec![1.0; max_lag + 1];
    let mut running = 0.0;
    for lag in 1..=max_lag {
        running += difference[lag];
        cmnd[lag] = if running <= f64::EPSILON {
            1.0
        } else {
            difference[lag] * lag as f64 / running
        };
    }

    let troughs = (min_lag..=max_lag)
        .filter(|&lag| {
            cmnd[lag] <= cmnd[lag.saturating_sub(1)]
                && (lag == max_lag || cmnd[lag] < cmnd[lag + 1])
        })
        .collect::<Vec<_>>();
    let energy_weight = (rms / (plan.yin.silence_rms.max(1e-12) * 8.0)).clamp(0.0, 1.0);
    let yin_lag = troughs
        .iter()
        .copied()
        .find(|&lag| cmnd[lag] <= plan.yin.threshold);
    let mut threshold_mass = vec![0.0; cmnd.len()];
    for threshold in &plan.yin.pyin_thresholds {
        if let Some(lag) = troughs.iter().copied().find(|&lag| cmnd[lag] <= *threshold) {
            threshold_mass[lag] += 1.0 / plan.yin.pyin_thresholds.len() as f64;
        }
    }
    let mut evaluated = troughs
        .into_iter()
        .map(|lag| {
            let voiced_probability = match plan.method {
                PitchTrackMethod::Yin => f64::from(Some(lag) == yin_lag),
                PitchTrackMethod::Pyin => threshold_mass[lag],
            } * energy_weight;
            hypothesis(
                lag,
                &cmnd,
                sample_rate,
                tuning,
                plan.interpolation,
                voiced_probability,
                energy_weight,
            )
        })
        .collect::<Vec<_>>();
    evaluated.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| left.lag.cmp(&right.lag))
    });

    let mut candidates = Vec::new();
    let mut rejected = Vec::new();
    for candidate in evaluated {
        let threshold_passed = match plan.method {
            PitchTrackMethod::Yin => Some(candidate.lag) == yin_lag,
            PitchTrackMethod::Pyin => {
                candidate.voiced_probability >= plan.yin.min_voiced_probability
            }
        };
        if !threshold_passed {
            rejected.push(RejectedPitchHypothesis {
                hypothesis: Some(candidate),
                reason: match plan.method {
                    PitchTrackMethod::Yin => PitchRejectionReason::Threshold,
                    PitchTrackMethod::Pyin => PitchRejectionReason::VoicedProbability,
                },
            });
        } else if candidates.len() == plan.control.max_results {
            rejected.push(RejectedPitchHypothesis {
                hypothesis: Some(candidate),
                reason: PitchRejectionReason::ResultLimit,
            });
        } else {
            candidates.push(candidate);
        }
    }
    Ok(PitchTrackFrame {
        provenance,
        candidates,
        rejected,
    })
}

fn hypothesis(
    lag: usize,
    cmnd: &[f64],
    sample_rate: u32,
    tuning: &dyn Tuning,
    interpolation: PitchInterpolation,
    voiced_probability: f64,
    energy_weight: f64,
) -> PitchHypothesis {
    let delta = match interpolation {
        PitchInterpolation::None => 0.0,
        PitchInterpolation::Parabolic if lag > 0 && lag + 1 < cmnd.len() => {
            parabolic_delta(cmnd[lag - 1], cmnd[lag], cmnd[lag + 1])
        }
        PitchInterpolation::Parabolic => 0.0,
    };
    let interpolated_lag = (lag as f64 + delta).max(1.0);
    let frequency = Frequency(f64::from(sample_rate) / interpolated_lag);
    let lower_frequency = Frequency(f64::from(sample_rate) / (interpolated_lag + 0.5));
    let upper_frequency = Frequency(f64::from(sample_rate) / (interpolated_lag - 0.5).max(0.5));
    let pitch = tuning.pitch_of(frequency);
    let periodicity = (1.0 - cmnd[lag]).clamp(0.0, 1.0);
    PitchHypothesis {
        pitch,
        frequency,
        lower_frequency,
        upper_frequency,
        lag,
        interpolated_lag,
        periodicity,
        voiced_probability,
        confidence: (periodicity * voiced_probability.sqrt() * energy_weight).clamp(0.0, 1.0),
        cents_error: frequency.cents_above(tuning.frequency_of(pitch)),
    }
}

fn parabolic_delta(left: f64, center: f64, right: f64) -> f64 {
    let denominator = left - 2.0 * center + right;
    if denominator.abs() <= f64::EPSILON {
        0.0
    } else {
        (0.5 * (left - right) / denominator).clamp(-0.5, 0.5)
    }
}

struct WorkMeter {
    limit: u64,
    used: u64,
}

impl WorkMeter {
    fn new(limit: u64) -> Self {
        Self { limit, used: 0 }
    }

    fn charge(&mut self, amount: u64) -> Result<(), AudioLiftError> {
        let next = self
            .used
            .checked_add(amount)
            .ok_or(AudioLiftError::PitchWorkLimit { limit: self.limit })?;
        if next > self.limit {
            return Err(AudioLiftError::PitchWorkLimit { limit: self.limit });
        }
        self.used = next;
        Ok(())
    }
}
