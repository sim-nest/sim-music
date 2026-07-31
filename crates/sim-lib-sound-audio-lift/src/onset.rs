//! Bounded onset-strength and peak-picking analysis.

use crate::{AudioTransformError, Stft, invalid};

/// Frame-to-frame novelty function used for onset strength.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnsetStrengthFunction {
    /// Half-wave-rectified growth in magnitude spectra.
    SpectralFlux,
    /// Half-wave-rectified growth in frame energy.
    EnergyDifference,
    /// Magnitude weighted by bin index, emphasizing high-frequency attacks.
    HighFrequencyContent,
}

/// Explicit onset-strength policy and deterministic work ceiling.
#[derive(Clone, Debug, PartialEq)]
pub struct OnsetStrengthPlan {
    /// Novelty function applied to each frame.
    pub function: OnsetStrengthFunction,
    /// Number of frames between the reference and current frame.
    pub lag_frames: usize,
    /// Normalize the completed curve so its strongest value is one.
    pub normalize: bool,
    /// Maximum complex-bin visits admitted by the analysis.
    pub max_work: u64,
}

impl Default for OnsetStrengthPlan {
    fn default() -> Self {
        Self {
            function: OnsetStrengthFunction::SpectralFlux,
            lag_frames: 1,
            normalize: true,
            max_work: 16_777_216,
        }
    }
}

/// One timestamped onset-strength value.
#[derive(Clone, Debug, PartialEq)]
pub struct OnsetStrengthFrame {
    /// Source STFT frame index.
    pub frame_index: usize,
    /// Source-sample coordinate represented by the frame center.
    pub sample: i64,
    /// Non-negative novelty strength.
    pub strength: f64,
}

/// Onset-strength curve retaining its complete policy and work evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct OnsetStrength {
    /// Plan used to calculate the curve.
    pub plan: OnsetStrengthPlan,
    /// Timestamped strength values.
    pub frames: Vec<OnsetStrengthFrame>,
    /// Complex-bin visits charged by the calculation.
    pub work_used: u64,
}

/// Policy for bounded local peak picking.
#[derive(Clone, Debug, PartialEq)]
pub struct PeakPickingPlan {
    /// Frames inspected before a candidate for the local-maximum test.
    pub local_max_before: usize,
    /// Frames inspected after a candidate; this is the algorithmic latency.
    pub local_max_after: usize,
    /// Frames inspected before a candidate for its adaptive mean floor.
    pub average_before: usize,
    /// Frames inspected after a candidate for its adaptive mean floor.
    pub average_after: usize,
    /// Amount a candidate must exceed the local mean.
    pub threshold: f64,
    /// Minimum admitted distance between selected onsets, in samples.
    pub minimum_distance_samples: usize,
    /// Maximum number of returned onset peaks.
    pub max_peaks: usize,
    /// Maximum frame visits admitted by peak picking.
    pub max_work: u64,
}

impl Default for PeakPickingPlan {
    fn default() -> Self {
        Self {
            local_max_before: 1,
            local_max_after: 1,
            average_before: 4,
            average_after: 4,
            threshold: 0.08,
            minimum_distance_samples: 1,
            max_peaks: 4_096,
            max_work: 1_000_000,
        }
    }
}

/// Why a reviewed onset hypothesis was not selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnsetRejection {
    /// The hypothesis did not clear the adaptive local-mean threshold.
    BelowThreshold,
    /// A stronger local hypothesis won the declared minimum-distance interval.
    MinimumDistance,
    /// The caller's result ceiling retained stronger hypotheses.
    ResultLimit,
}

/// One selected onset with confidence and latency evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct OnsetPeak {
    /// Source onset-strength frame index.
    pub frame_index: usize,
    /// Detected source-sample coordinate.
    pub sample: i64,
    /// Earliest sample coordinate at which lookahead makes this decision final.
    pub available_at_sample: i64,
    /// Original onset strength.
    pub strength: f64,
    /// Strength relative to the strongest local maximum in this run.
    pub confidence: f64,
}

/// A retained but rejected onset alternative.
#[derive(Clone, Debug, PartialEq)]
pub struct OnsetAlternative {
    /// Candidate that was reviewed.
    pub candidate: OnsetPeak,
    /// Explicit policy reason for rejection.
    pub reason: OnsetRejection,
}

/// Selected onset peaks plus every reviewed alternative.
#[derive(Clone, Debug, PartialEq)]
pub struct OnsetPeaks {
    /// Complete picking policy.
    pub plan: PeakPickingPlan,
    /// Algorithmic lookahead latency in source samples.
    pub latency_samples: usize,
    /// Selected peaks in chronological order.
    pub peaks: Vec<OnsetPeak>,
    /// Rejected local-maximum hypotheses.
    pub alternatives: Vec<OnsetAlternative>,
    /// Frame visits charged by the picker.
    pub work_used: u64,
}

/// Computes a bounded onset-strength curve from phase-preserving STFT frames.
pub fn onset_strength(
    analysis: &Stft,
    plan: &OnsetStrengthPlan,
) -> Result<OnsetStrength, AudioTransformError> {
    if plan.lag_frames == 0 {
        return Err(invalid("onset lag", "lag must be at least one frame"));
    }
    if plan.max_work == 0 {
        return Err(invalid("onset work", "work limit must be positive"));
    }
    let bins = analysis.plan.frame / 2 + 1;
    let required = analysis
        .frames
        .len()
        .checked_mul(bins)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| work_limit(u64::MAX, plan.max_work))?;
    if required > plan.max_work {
        return Err(work_limit(required, plan.max_work));
    }

    let magnitude = analysis
        .frames
        .iter()
        .map(|frame| {
            frame
                .bins
                .iter()
                .map(|(real, imaginary)| real.hypot(*imaginary))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut frames = Vec::with_capacity(analysis.frames.len());
    for (index, frame) in analysis.frames.iter().enumerate() {
        let strength = index.checked_sub(plan.lag_frames).map_or(0.0, |previous| {
            novelty(plan.function, &magnitude[previous], &magnitude[index])
        });
        frames.push(OnsetStrengthFrame {
            frame_index: frame.index,
            sample: frame.center_sample,
            strength,
        });
    }
    if plan.normalize {
        let maximum = frames
            .iter()
            .map(|frame| frame.strength)
            .fold(0.0_f64, f64::max);
        if maximum > f64::EPSILON {
            for frame in &mut frames {
                frame.strength /= maximum;
            }
        }
    }
    Ok(OnsetStrength {
        plan: plan.clone(),
        frames,
        work_used: required,
    })
}

/// Selects bounded onset peaks under explicit lookahead and distance policy.
pub fn pick_onsets(
    curve: &OnsetStrength,
    hop_samples: usize,
    plan: &PeakPickingPlan,
) -> Result<OnsetPeaks, AudioTransformError> {
    validate_picker(plan, hop_samples)?;
    let latency_samples = plan
        .local_max_after
        .checked_mul(hop_samples)
        .ok_or_else(|| work_limit(u64::MAX, plan.max_work))?;
    let mut work_used = 0_u64;
    let mut local = Vec::new();
    let mut below = Vec::new();
    for index in 0..curve.frames.len() {
        let maximum = bounded_range(
            index,
            plan.local_max_before,
            plan.local_max_after,
            curve.frames.len(),
        );
        let average = bounded_range(
            index,
            plan.average_before,
            plan.average_after,
            curve.frames.len(),
        );
        charge(&mut work_used, maximum.len() + average.len(), plan.max_work)?;
        let frame = &curve.frames[index];
        let is_maximum = maximum
            .clone()
            .all(|other| curve.frames[other].strength <= frame.strength);
        if !is_maximum || frame.strength <= 0.0 {
            continue;
        }
        let mean = average
            .clone()
            .map(|other| curve.frames[other].strength)
            .sum::<f64>()
            / average.len() as f64;
        if frame.strength >= mean + plan.threshold {
            local.push(index);
        } else {
            below.push(index);
        }
    }
    let strongest = local
        .iter()
        .chain(&below)
        .map(|&index| curve.frames[index].strength)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);
    local.sort_by(|left, right| {
        curve.frames[*right]
            .strength
            .total_cmp(&curve.frames[*left].strength)
            .then_with(|| left.cmp(right))
    });
    let mut selected = Vec::<usize>::new();
    let mut alternatives = below
        .into_iter()
        .map(|index| {
            alternative(
                curve,
                index,
                latency_samples,
                strongest,
                OnsetRejection::BelowThreshold,
            )
        })
        .collect::<Vec<_>>();
    for index in local {
        let sample = curve.frames[index].sample;
        if selected.iter().any(|&other| {
            curve.frames[other].sample.abs_diff(sample) < plan.minimum_distance_samples as u64
        }) {
            alternatives.push(alternative(
                curve,
                index,
                latency_samples,
                strongest,
                OnsetRejection::MinimumDistance,
            ));
        } else if selected.len() == plan.max_peaks {
            alternatives.push(alternative(
                curve,
                index,
                latency_samples,
                strongest,
                OnsetRejection::ResultLimit,
            ));
        } else {
            selected.push(index);
        }
    }
    selected.sort_unstable();
    alternatives.sort_by_key(|item| item.candidate.sample);
    let peaks = selected
        .into_iter()
        .map(|index| peak(curve, index, latency_samples, strongest))
        .collect();
    Ok(OnsetPeaks {
        plan: plan.clone(),
        latency_samples,
        peaks,
        alternatives,
        work_used,
    })
}

fn novelty(function: OnsetStrengthFunction, previous: &[f64], current: &[f64]) -> f64 {
    match function {
        OnsetStrengthFunction::SpectralFlux => previous
            .iter()
            .zip(current)
            .map(|(left, right)| (right - left).max(0.0).powi(2))
            .sum::<f64>()
            .sqrt(),
        OnsetStrengthFunction::EnergyDifference => {
            let before = previous.iter().map(|value| value * value).sum::<f64>();
            let after = current.iter().map(|value| value * value).sum::<f64>();
            (after - before).max(0.0).sqrt()
        }
        OnsetStrengthFunction::HighFrequencyContent => current
            .iter()
            .enumerate()
            .map(|(index, value)| (index + 1) as f64 * value * value)
            .sum::<f64>()
            .sqrt(),
    }
}

fn validate_picker(plan: &PeakPickingPlan, hop: usize) -> Result<(), AudioTransformError> {
    if hop == 0 || plan.max_peaks == 0 || plan.max_work == 0 {
        return Err(invalid(
            "onset peak picking",
            "hop, result limit, and work limit must be positive",
        ));
    }
    if !plan.threshold.is_finite() || plan.threshold < 0.0 {
        return Err(invalid(
            "onset threshold",
            "threshold must be finite and non-negative",
        ));
    }
    Ok(())
}

fn bounded_range(index: usize, before: usize, after: usize, len: usize) -> std::ops::Range<usize> {
    index.saturating_sub(before)..index.saturating_add(after).saturating_add(1).min(len)
}

fn peak(curve: &OnsetStrength, index: usize, latency: usize, maximum: f64) -> OnsetPeak {
    let frame = &curve.frames[index];
    OnsetPeak {
        frame_index: frame.frame_index,
        sample: frame.sample,
        available_at_sample: frame.sample.saturating_add_unsigned(latency as u64),
        strength: frame.strength,
        confidence: (frame.strength / maximum).clamp(0.0, 1.0),
    }
}

fn alternative(
    curve: &OnsetStrength,
    index: usize,
    latency: usize,
    maximum: f64,
    reason: OnsetRejection,
) -> OnsetAlternative {
    OnsetAlternative {
        candidate: peak(curve, index, latency, maximum),
        reason,
    }
}

fn charge(used: &mut u64, amount: usize, limit: u64) -> Result<(), AudioTransformError> {
    let amount = u64::try_from(amount).map_err(|_| work_limit(u64::MAX, limit))?;
    *used = used
        .checked_add(amount)
        .ok_or_else(|| work_limit(u64::MAX, limit))?;
    if *used > limit {
        return Err(work_limit(*used, limit));
    }
    Ok(())
}

fn work_limit(required: u64, maximum: u64) -> AudioTransformError {
    AudioTransformError::WorkLimit {
        resource: "onset analysis",
        required,
        maximum,
    }
}
