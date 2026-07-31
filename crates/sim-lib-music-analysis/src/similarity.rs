//! Melody and rhythm similarity through shared DTW and correlation engines.

use sim_lib_discrete_graph::{
    AlgorithmControl, Alignment, AlignmentBoundary, AlignmentMemory, AlignmentWindow, DtwPolicy,
    GapPolicy, dynamic_time_warp_with_control,
};
use sim_lib_music_core::Time;
use sim_lib_numbers_signal::{
    CorrelationNormalization, CorrelationPlan, CorrelationResult, LagOrder, correlate,
};

use crate::{AnalysisError, AnalysisEvent, AnalysisTransform, ratio_to_f64, sequence_extent};

/// Named scalar feature extracted before sequence comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimilarityFeature {
    /// Absolute chromatic pitch in semitones.
    AbsolutePitch,
    /// Signed semitone movement between adjacent notes.
    PitchContour,
    /// Exact onset interval between adjacent events.
    InterOnsetRhythm,
    /// Exact event duration.
    DurationRhythm,
}

/// Explicit invariances admitted while comparing the right sequence to the left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimilarityInvariances {
    /// Admit one inferred chromatic transposition.
    pub transposition: bool,
    /// Admit one inferred exact uniform time scale.
    pub time_scale: bool,
}

/// Feature, invariance, and delegated algorithm policy.
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarityPlan {
    /// Named feature extractor.
    pub feature: SimilarityFeature,
    /// Transform classes the comparison may consider.
    pub invariances: SimilarityInvariances,
    /// One-sided DTW gap cost.
    pub gap_cost: f64,
    /// Optional Sakoe-Chiba-style prefix radius.
    pub alignment_window: AlignmentWindow,
    /// DTW work and memory limits.
    pub control: AlgorithmControl,
    /// Maximum transform alternatives retained after stable cost ranking.
    pub max_alternatives: usize,
}

/// Correlation peak and the complete generic correlation result.
#[derive(Clone, Debug, PartialEq)]
pub struct CorrelationEvidence {
    /// Signed feature displacement at the maximum coefficient.
    pub best_lag: isize,
    /// Maximum normalized cross-correlation coefficient.
    pub coefficient: f64,
    /// Samples, lags, convolution selection, and normalization evidence.
    pub result: CorrelationResult,
}

/// One exact transform alternative with both generic algorithm results.
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarityAlternative {
    /// Affine transform mapping the right sequence toward the left.
    pub transform: AnalysisTransform,
    /// Combined, lower-is-better DTW/correlation cost.
    pub cost: f64,
    /// Certified generic dynamic-time-warp result.
    pub alignment: Alignment<f64>,
    /// Generic normalized cross-correlation result and selected peak.
    pub correlation: CorrelationEvidence,
}

/// Ranked melody/rhythm similarity with declared semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct SimilarityReport {
    /// Complete extractor, invariance, alignment, and alternative policy.
    pub plan: SimilarityPlan,
    /// Named extractor used for both sequences.
    pub feature: SimilarityFeature,
    /// Invariances considered, never inferred ambiently.
    pub invariances: SimilarityInvariances,
    /// Best-first stable transform alternatives.
    pub alternatives: Vec<SimilarityAlternative>,
}

impl SimilarityReport {
    /// Returns the selected lowest-cost alternative.
    pub fn selected(&self) -> &SimilarityAlternative {
        self.alternatives
            .first()
            .expect("successful similarity reports retain an alternative")
    }
}

/// Compares two identity-bearing event sequences through DTW and correlation.
pub fn compare_sequences(
    left: &[AnalysisEvent],
    right: &[AnalysisEvent],
    plan: &SimilarityPlan,
) -> Result<SimilarityReport, AnalysisError> {
    validate_plan(left, right, plan)?;
    let left = ordered(left);
    let right = ordered(right);
    let mut transforms = vec![AnalysisTransform::identity()];
    let inferred = infer_transform(&left, &right, plan.invariances)?;
    if inferred != transforms[0] {
        transforms.push(inferred);
    }

    let mut alternatives = transforms
        .into_iter()
        .map(|transform| compare_transform(&left, &right, plan, transform))
        .collect::<Result<Vec<_>, _>>()?;
    alternatives.sort_by(|left, right| {
        left.cost
            .total_cmp(&right.cost)
            .then_with(|| {
                left.transform
                    .transposition
                    .cmp(&right.transform.transposition)
            })
            .then_with(|| left.transform.time_scale.cmp(&right.transform.time_scale))
            .then_with(|| left.transform.time_shift.cmp(&right.transform.time_shift))
    });
    alternatives.truncate(plan.max_alternatives);
    Ok(SimilarityReport {
        plan: plan.clone(),
        feature: plan.feature,
        invariances: plan.invariances,
        alternatives,
    })
}

fn validate_plan(
    left: &[AnalysisEvent],
    right: &[AnalysisEvent],
    plan: &SimilarityPlan,
) -> Result<(), AnalysisError> {
    if left.is_empty() || right.is_empty() {
        return Err(AnalysisError::InvalidInput {
            field: "similarity sequences",
            reason: "both sequences must contain at least one event".to_owned(),
        });
    }
    if matches!(
        plan.feature,
        SimilarityFeature::PitchContour | SimilarityFeature::InterOnsetRhythm
    ) && (left.len() < 2 || right.len() < 2)
    {
        return Err(AnalysisError::InvalidInput {
            field: "similarity sequences",
            reason: "adjacent-event features require at least two events per sequence".to_owned(),
        });
    }
    if !plan.gap_cost.is_finite() || plan.gap_cost < 0.0 {
        return Err(AnalysisError::InvalidPolicy {
            field: "gap_cost",
            reason: "gap cost must be finite and non-negative".to_owned(),
        });
    }
    if plan.max_alternatives == 0 {
        return Err(AnalysisError::InvalidPolicy {
            field: "max_alternatives",
            reason: "at least one transform alternative is required".to_owned(),
        });
    }
    Ok(())
}

fn ordered(events: &[AnalysisEvent]) -> Vec<AnalysisEvent> {
    let mut events = events.to_vec();
    events.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| left.pitch.cmp(&right.pitch))
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    events
}

fn infer_transform(
    left: &[AnalysisEvent],
    right: &[AnalysisEvent],
    invariances: SimilarityInvariances,
) -> Result<AnalysisTransform, AnalysisError> {
    let transposition = if invariances.transposition {
        let mut offsets = left
            .iter()
            .zip(right)
            .map(|(left, right)| left.pitch.semitone() - right.pitch.semitone())
            .collect::<Vec<_>>();
        offsets.sort_unstable();
        offsets[offsets.len() / 2]
    } else {
        0
    };
    let time_scale = if invariances.time_scale {
        let left_extent = sequence_extent(left);
        let right_extent = sequence_extent(right);
        if right_extent > Time::from_integer(0) {
            left_extent / right_extent
        } else {
            Time::from_integer(1)
        }
    } else {
        Time::from_integer(1)
    };
    if time_scale <= Time::from_integer(0) {
        return Err(AnalysisError::InvalidInput {
            field: "similarity sequence extent",
            reason: "inferred time scale must be positive".to_owned(),
        });
    }
    let time_shift = left[0].onset - right[0].onset * time_scale;
    Ok(AnalysisTransform {
        transposition,
        time_scale,
        time_shift,
    })
}

fn compare_transform(
    left: &[AnalysisEvent],
    right: &[AnalysisEvent],
    plan: &SimilarityPlan,
    transform: AnalysisTransform,
) -> Result<SimilarityAlternative, AnalysisError> {
    let left_features = extract(left, plan.feature, &AnalysisTransform::identity())?;
    let right_features = extract(right, plan.feature, &transform)?;
    let policy = DtwPolicy::new(GapPolicy::new(plan.gap_cost, plan.gap_cost))
        .with_boundary(AlignmentBoundary::Global)
        .with_window(plan.alignment_window)
        .with_memory(AlignmentMemory::Full);
    let alignment = dynamic_time_warp_with_control(
        &left_features,
        &right_features,
        |left, right| (left - right).abs(),
        policy,
        &plan.control,
        &sim_lib_discrete_graph::NeverInterrupt,
    )
    .map_err(|error| AnalysisError::Alignment(error.to_string()))?;
    let correlation_plan = CorrelationPlan {
        normalization: CorrelationNormalization::Coefficient,
        lag_order: LagOrder::Ascending,
        ..CorrelationPlan::linear_full()
    };
    let result = correlate(&left_features, &right_features, &correlation_plan)
        .map_err(|error| AnalysisError::Correlation(error.to_string()))?;
    let (best_index, coefficient) = result
        .samples
        .as_slice()
        .iter()
        .copied()
        .enumerate()
        .max_by(|(left_index, left), (right_index, right)| {
            left.total_cmp(right)
                .then_with(|| right_index.cmp(left_index))
        })
        .expect("non-empty features produce correlation samples");
    let normalized_dtw = alignment.score / left_features.len().max(right_features.len()) as f64;
    let cost = normalized_dtw + (1.0 - coefficient.clamp(-1.0, 1.0));
    Ok(SimilarityAlternative {
        transform,
        cost,
        alignment,
        correlation: CorrelationEvidence {
            best_lag: result.lags[best_index],
            coefficient,
            result,
        },
    })
}

fn extract(
    events: &[AnalysisEvent],
    feature: SimilarityFeature,
    transform: &AnalysisTransform,
) -> Result<Vec<f64>, AnalysisError> {
    match feature {
        SimilarityFeature::AbsolutePitch => events
            .iter()
            .map(|event| Ok(f64::from(event.pitch.semitone() + transform.transposition)))
            .collect(),
        SimilarityFeature::PitchContour => events
            .windows(2)
            .map(|pair| {
                Ok(f64::from(
                    pair[1].pitch.semitone() - pair[0].pitch.semitone(),
                ))
            })
            .collect(),
        SimilarityFeature::InterOnsetRhythm => events
            .windows(2)
            .map(|pair| ratio_to_f64((pair[1].onset - pair[0].onset) * transform.time_scale))
            .collect(),
        SimilarityFeature::DurationRhythm => events
            .iter()
            .map(|event| ratio_to_f64(event.duration * transform.time_scale))
            .collect(),
    }
}
