//! Policy-explicit framed zero-crossing rate.

use crate::{AudioTransformError, invalid};

/// Treatment of exact or near-zero samples in zero-crossing analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroCrossingPolicy {
    /// Carry the most recent nonzero sign through zeros.
    CarryPrevious,
    /// Treat zeros as non-negative samples.
    Positive,
    /// Exclude pairs containing a zero from the crossing denominator.
    Ignore,
}

/// Framing and zero policy for time-domain crossing rate.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeroCrossingPlan {
    /// Samples in one frame.
    pub frame: usize,
    /// Samples advanced between frames.
    pub hop: usize,
    /// Absolute values at or below this threshold count as zero.
    pub zero_threshold: f64,
    /// Declared treatment of zero samples.
    pub zero_policy: ZeroCrossingPolicy,
    /// Maximum sample-pair visits.
    pub max_work: u64,
}

impl Default for ZeroCrossingPlan {
    fn default() -> Self {
        Self {
            frame: 2_048,
            hop: 512,
            zero_threshold: 0.0,
            zero_policy: ZeroCrossingPolicy::CarryPrevious,
            max_work: 16_777_216,
        }
    }
}

/// One timestamped zero-crossing rate.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeroCrossingFrame {
    /// Zero-based frame index.
    pub index: usize,
    /// Source-sample coordinate of the frame start.
    pub onset_sample: usize,
    /// Crossings divided by reviewed adjacent pairs.
    pub rate: f64,
    /// Adjacent sample pairs admitted by the zero policy.
    pub reviewed_pairs: usize,
}

/// Framed zero-crossing output with full source-rate and policy evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeroCrossingRate {
    /// Full source sample rate.
    pub sample_rate: u32,
    /// Complete framing and zero policy.
    pub plan: ZeroCrossingPlan,
    /// Timestamped rates.
    pub frames: Vec<ZeroCrossingFrame>,
    /// Sample-pair visits charged by the calculation.
    pub work_used: u64,
}

/// Computes a bounded, policy-explicit framed zero-crossing rate.
pub fn zero_crossing_rate(
    samples: &[f32],
    sample_rate: u32,
    plan: &ZeroCrossingPlan,
) -> Result<ZeroCrossingRate, AudioTransformError> {
    validate(samples, sample_rate, plan)?;
    let starts = (0..samples.len()).step_by(plan.hop).collect::<Vec<_>>();
    let required = starts
        .len()
        .checked_mul(plan.frame - 1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| work_limit(u64::MAX, plan.max_work))?;
    if required > plan.max_work {
        return Err(work_limit(required, plan.max_work));
    }
    let frames = starts
        .into_iter()
        .enumerate()
        .map(|(index, start)| crossing_frame(samples, start, index, plan))
        .collect();
    Ok(ZeroCrossingRate {
        sample_rate,
        plan: plan.clone(),
        frames,
        work_used: required,
    })
}

fn validate(
    samples: &[f32],
    sample_rate: u32,
    plan: &ZeroCrossingPlan,
) -> Result<(), AudioTransformError> {
    if sample_rate == 0
        || plan.frame < 2
        || plan.hop == 0
        || !plan.zero_threshold.is_finite()
        || plan.zero_threshold < 0.0
        || plan.max_work == 0
    {
        return Err(invalid(
            "zero-crossing plan",
            "sample rate, frame, hop, threshold, and work policy are invalid",
        ));
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid("zero-crossing samples", "samples must be finite"));
    }
    Ok(())
}

fn crossing_frame(
    samples: &[f32],
    start: usize,
    index: usize,
    plan: &ZeroCrossingPlan,
) -> ZeroCrossingFrame {
    let end = start.saturating_add(plan.frame).min(samples.len());
    let mut previous = None;
    let mut crossings = 0usize;
    let mut reviewed = 0usize;
    for sample in &samples[start..end] {
        let sign = sample_sign(f64::from(*sample), plan.zero_threshold);
        match plan.zero_policy {
            ZeroCrossingPolicy::CarryPrevious => {
                if let Some(sign) = sign {
                    review_sign(sign, &mut previous, &mut crossings, &mut reviewed);
                }
            }
            ZeroCrossingPolicy::Positive => {
                review_sign(
                    sign.unwrap_or(true),
                    &mut previous,
                    &mut crossings,
                    &mut reviewed,
                );
            }
            ZeroCrossingPolicy::Ignore => {
                if let Some(sign) = sign {
                    review_sign(sign, &mut previous, &mut crossings, &mut reviewed);
                } else {
                    previous = None;
                }
            }
        }
    }
    ZeroCrossingFrame {
        index,
        onset_sample: start,
        rate: crossings as f64 / reviewed.max(1) as f64,
        reviewed_pairs: reviewed,
    }
}

fn review_sign(
    sign: bool,
    previous: &mut Option<bool>,
    crossings: &mut usize,
    reviewed: &mut usize,
) {
    if previous.is_some_and(|previous| previous != sign) {
        *crossings += 1;
    }
    if previous.is_some() {
        *reviewed += 1;
    }
    *previous = Some(sign);
}

fn sample_sign(value: f64, threshold: f64) -> Option<bool> {
    (value.abs() > threshold).then_some(value >= 0.0)
}

fn work_limit(required: u64, maximum: u64) -> AudioTransformError {
    AudioTransformError::WorkLimit {
        resource: "zero-crossing rate",
        required,
        maximum,
    }
}
