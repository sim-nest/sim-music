//! Tempo hypotheses and certified varying-tempo beat tracking.

use std::collections::BTreeMap;

use sim_lib_discrete_graph::{
    AlgorithmControl, AlgorithmReceipt, LayeredCertificate, NeverInterrupt,
    layered_shortest_path_with_control,
};

use crate::{AudioTransformError, OnsetPeak, OnsetPeaks, invalid};

/// Explicit tempo, dynamic-programming, meter, and resource policy.
#[derive(Clone, Debug, PartialEq)]
pub struct BeatTrackingPlan {
    /// Lowest admitted tempo in beats per minute.
    pub minimum_bpm: f64,
    /// Highest admitted tempo in beats per minute.
    pub maximum_bpm: f64,
    /// Half-width of one global tempo histogram bucket in BPM.
    pub tempo_bucket_bpm: f64,
    /// Maximum global tempo alternatives returned.
    pub max_tempo_candidates: usize,
    /// Penalty for moving between local tempi; zero allows unconstrained changes.
    pub tempo_change_penalty: f64,
    /// Weight of each local onset/interval observation in the staged objective.
    pub observation_weight: f64,
    /// Meter numerators to review at every possible phase.
    pub meters: Vec<u8>,
    /// Maximum work charged by generic layered dynamic programming.
    pub max_work: u64,
    /// Maximum retained staged-DP cells.
    pub max_memory_cells: usize,
}

impl Default for BeatTrackingPlan {
    fn default() -> Self {
        Self {
            minimum_bpm: 40.0,
            maximum_bpm: 240.0,
            tempo_bucket_bpm: 1.0,
            max_tempo_candidates: 16,
            tempo_change_penalty: 1.5,
            observation_weight: 1.0,
            meters: vec![2, 3, 4, 6],
            max_work: 1_000_000,
            max_memory_cells: 100_000,
        }
    }
}

/// One retained global tempo alternative.
#[derive(Clone, Debug, PartialEq)]
pub struct TempoCandidate {
    /// Weighted mean tempo of this histogram bucket.
    pub bpm: f64,
    /// Normalized evidence mass in `0.0..=1.0`.
    pub confidence: f64,
    /// Count of inter-onset hypotheses supporting the bucket.
    pub support: usize,
}

/// One local tempo alternative considered after an onset.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalTempoAlternative {
    /// Tempo in beats per minute.
    pub bpm: f64,
    /// Observation confidence before trajectory smoothing.
    pub confidence: f64,
    /// Multiplicative metrical interpretation of the observed interval.
    pub interval_factor: f64,
}

/// One detected beat retaining its local tempo and alternatives.
#[derive(Clone, Debug, PartialEq)]
pub struct Beat {
    /// Source-sample coordinate of the beat.
    pub sample: i64,
    /// Confidence inherited from the onset and selected interval hypothesis.
    pub confidence: f64,
    /// Selected local tempo; absent for the first beat.
    pub bpm: Option<f64>,
    /// Every reviewed local tempo interpretation, strongest first.
    pub alternatives: Vec<LocalTempoAlternative>,
}

/// One retained meter and phase hypothesis.
#[derive(Clone, Debug, PartialEq)]
pub struct MeterHypothesis {
    /// Beats in one hypothesized bar.
    pub beats_per_bar: u8,
    /// Zero-based beat within the bar assigned to the first detected beat.
    pub phase: u8,
    /// Relative accent evidence in `0.0..=1.0`.
    pub confidence: f64,
}

/// Certified staged-DP evidence for the selected varying-tempo trajectory.
#[derive(Clone, Debug, PartialEq)]
pub struct BeatDpEvidence {
    /// Stable state index selected in each interval layer.
    pub selected_indices: Vec<usize>,
    /// Finite total observation and tempo-change cost.
    pub total_cost: f64,
    /// Generic Bellman recurrence and backpointer certificate.
    pub certificate: LayeredCertificate<f64>,
    /// Deterministic generic graph work receipt.
    pub receipt: AlgorithmReceipt,
}

/// Beat sequence with global tempo, local tempo, meter, and alternatives.
#[derive(Clone, Debug, PartialEq)]
pub struct BeatTracking {
    /// Complete analysis policy.
    pub plan: BeatTrackingPlan,
    /// Global tempo alternatives aggregated across the sequence.
    pub tempo_candidates: Vec<TempoCandidate>,
    /// Chronological beat decisions with changing tempo retained.
    pub beats: Vec<Beat>,
    /// Meter/phase alternatives, strongest first.
    pub meter_hypotheses: Vec<MeterHypothesis>,
    /// Index of the selected meter hypothesis when evidence exists.
    pub selected_meter: Option<usize>,
    /// Generic staged-DP certificate; absent when fewer than two onsets exist.
    pub dynamic_programming: Option<BeatDpEvidence>,
}

#[derive(Clone, Debug, PartialEq)]
enum TempoNode {
    Root,
    Candidate(LocalTempoAlternative),
}

/// Tracks onset peaks as beats while preserving a changing local tempo path.
///
/// The music adapter prepares local metrical alternatives, then delegates the
/// bounded Bellman recurrence and certificate to `sim-lib-discrete-graph`.
pub fn track_beats(
    onsets: &OnsetPeaks,
    sample_rate: u32,
    plan: &BeatTrackingPlan,
) -> Result<BeatTracking, AudioTransformError> {
    validate_plan(plan, sample_rate)?;
    if onsets.peaks.is_empty() {
        return Ok(BeatTracking {
            plan: plan.clone(),
            tempo_candidates: Vec::new(),
            beats: Vec::new(),
            meter_hypotheses: Vec::new(),
            selected_meter: None,
            dynamic_programming: None,
        });
    }
    let local = onsets
        .peaks
        .windows(2)
        .map(|pair| interval_hypotheses(&pair[0], &pair[1], sample_rate, plan))
        .collect::<Result<Vec<_>, _>>()?;
    let tempo_candidates = aggregate_tempi(&local, plan);
    if local.is_empty() {
        return Ok(BeatTracking {
            plan: plan.clone(),
            tempo_candidates,
            beats: vec![Beat {
                sample: onsets.peaks[0].sample,
                confidence: onsets.peaks[0].confidence,
                bpm: None,
                alternatives: Vec::new(),
            }],
            meter_hypotheses: Vec::new(),
            selected_meter: None,
            dynamic_programming: None,
        });
    }

    let mut layers = vec![vec![TempoNode::Root]];
    layers.extend(local.iter().cloned().map(|layer| {
        layer
            .into_iter()
            .map(TempoNode::Candidate)
            .collect::<Vec<_>>()
    }));
    let graph_control = AlgorithmControl::default()
        .with_max_work(plan.max_work)
        .with_max_memory_cells(plan.max_memory_cells);
    let path = layered_shortest_path_with_control(
        &layers,
        |left, right| transition_cost(left, right, plan),
        &graph_control,
        &NeverInterrupt,
    )
    .map_err(|error| AudioTransformError::Graph(error.to_string()))?;
    let selected = path
        .states
        .iter()
        .skip(1)
        .map(|state| match state {
            TempoNode::Candidate(candidate) => candidate,
            TempoNode::Root => unreachable!("root appears only in the first layer"),
        })
        .collect::<Vec<_>>();
    let mut beats = Vec::with_capacity(onsets.peaks.len());
    beats.push(Beat {
        sample: onsets.peaks[0].sample,
        confidence: onsets.peaks[0].confidence,
        bpm: None,
        alternatives: Vec::new(),
    });
    for (index, candidate) in selected.into_iter().enumerate() {
        let onset = &onsets.peaks[index + 1];
        beats.push(Beat {
            sample: onset.sample,
            confidence: onset.confidence.min(candidate.confidence),
            bpm: Some(candidate.bpm),
            alternatives: local[index].clone(),
        });
    }
    let meter_hypotheses = infer_meter(&onsets.peaks, plan);
    let selected_meter = (!meter_hypotheses.is_empty()).then_some(0);
    Ok(BeatTracking {
        plan: plan.clone(),
        tempo_candidates,
        beats,
        meter_hypotheses,
        selected_meter,
        dynamic_programming: Some(BeatDpEvidence {
            selected_indices: path.indices[1..].to_vec(),
            total_cost: path.total_cost,
            certificate: path.certificate,
            receipt: path.receipt,
        }),
    })
}

fn validate_plan(plan: &BeatTrackingPlan, sample_rate: u32) -> Result<(), AudioTransformError> {
    if sample_rate == 0 {
        return Err(invalid("beat sample rate", "sample rate must be positive"));
    }
    if !plan.minimum_bpm.is_finite()
        || !plan.maximum_bpm.is_finite()
        || plan.minimum_bpm <= 0.0
        || plan.minimum_bpm >= plan.maximum_bpm
    {
        return Err(invalid(
            "beat tempo range",
            "finite positive bounds must be in ascending order",
        ));
    }
    if !plan.tempo_bucket_bpm.is_finite()
        || plan.tempo_bucket_bpm <= 0.0
        || !plan.tempo_change_penalty.is_finite()
        || plan.tempo_change_penalty < 0.0
        || !plan.observation_weight.is_finite()
        || plan.observation_weight < 0.0
    {
        return Err(invalid(
            "beat cost policy",
            "bucket width and finite non-negative costs are required",
        ));
    }
    if plan.max_tempo_candidates == 0
        || plan.max_work == 0
        || plan.max_memory_cells == 0
        || plan.meters.iter().any(|meter| *meter < 2)
    {
        return Err(invalid(
            "beat bounds",
            "result/work/memory bounds must be positive and meters at least two",
        ));
    }
    Ok(())
}

fn interval_hypotheses(
    left: &OnsetPeak,
    right: &OnsetPeak,
    sample_rate: u32,
    plan: &BeatTrackingPlan,
) -> Result<Vec<LocalTempoAlternative>, AudioTransformError> {
    let distance = right
        .sample
        .checked_sub(left.sample)
        .ok_or_else(|| invalid("beat onsets", "onset samples must be strictly increasing"))?;
    if distance <= 0 {
        return Err(invalid(
            "beat onsets",
            "onset samples must be strictly increasing",
        ));
    }
    let observed = 60.0 * f64::from(sample_rate) / distance as f64;
    let mut alternatives = [0.5, 1.0, 2.0, 3.0]
        .into_iter()
        .filter_map(|factor| {
            let bpm = observed * factor;
            (plan.minimum_bpm..=plan.maximum_bpm)
                .contains(&bpm)
                .then_some(LocalTempoAlternative {
                    bpm,
                    confidence: (left.confidence * right.confidence).sqrt()
                        * metrical_prior(factor),
                    interval_factor: factor,
                })
        })
        .collect::<Vec<_>>();
    alternatives.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| left.bpm.total_cmp(&right.bpm))
    });
    if alternatives.is_empty() {
        return Err(invalid(
            "beat tempo range",
            "an inter-onset interval has no admitted metrical interpretation",
        ));
    }
    Ok(alternatives)
}

fn metrical_prior(factor: f64) -> f64 {
    if (factor - 1.0).abs() <= f64::EPSILON {
        1.0
    } else if factor == 0.5 || factor == 2.0 {
        0.8
    } else {
        0.6
    }
}

fn transition_cost(left: &TempoNode, right: &TempoNode, plan: &BeatTrackingPlan) -> Option<f64> {
    let TempoNode::Candidate(right) = right else {
        return None;
    };
    let observation = (1.0 - right.confidence.clamp(0.0, 1.0)) * plan.observation_weight;
    match left {
        TempoNode::Root => Some(observation),
        TempoNode::Candidate(left) => {
            Some(observation + (right.bpm / left.bpm).ln().abs() * plan.tempo_change_penalty)
        }
    }
}

fn aggregate_tempi(
    local: &[Vec<LocalTempoAlternative>],
    plan: &BeatTrackingPlan,
) -> Vec<TempoCandidate> {
    let mut buckets = BTreeMap::<i64, (f64, f64, usize)>::new();
    for hypothesis in local.iter().flatten() {
        let bucket = (hypothesis.bpm / plan.tempo_bucket_bpm).round() as i64;
        let entry = buckets.entry(bucket).or_default();
        entry.0 += hypothesis.bpm * hypothesis.confidence;
        entry.1 += hypothesis.confidence;
        entry.2 += 1;
    }
    let maximum = buckets
        .values()
        .map(|(_, confidence, _)| *confidence)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);
    let mut candidates = buckets
        .into_values()
        .map(|(weighted_bpm, confidence, support)| TempoCandidate {
            bpm: weighted_bpm / confidence.max(f64::EPSILON),
            confidence: (confidence / maximum).clamp(0.0, 1.0),
            support,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| left.bpm.total_cmp(&right.bpm))
    });
    candidates.truncate(plan.max_tempo_candidates);
    candidates
}

fn infer_meter(onsets: &[OnsetPeak], plan: &BeatTrackingPlan) -> Vec<MeterHypothesis> {
    if onsets.len() < 2 {
        return Vec::new();
    }
    let mut hypotheses = Vec::new();
    for &meter in &plan.meters {
        for phase in 0..meter {
            let mut downbeat = 0.0;
            let mut other = 0.0;
            let mut downbeat_count = 0usize;
            let mut other_count = 0usize;
            for (index, onset) in onsets.iter().enumerate() {
                if (index + usize::from(phase)).is_multiple_of(usize::from(meter)) {
                    downbeat += onset.strength;
                    downbeat_count += 1;
                } else {
                    other += onset.strength;
                    other_count += 1;
                }
            }
            let downbeat = downbeat / downbeat_count.max(1) as f64;
            let other = other / other_count.max(1) as f64;
            let confidence = if downbeat + other <= f64::EPSILON {
                0.0
            } else {
                (downbeat / (downbeat + other)).clamp(0.0, 1.0)
            };
            hypotheses.push(MeterHypothesis {
                beats_per_bar: meter,
                phase,
                confidence,
            });
        }
    }
    hypotheses.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| left.beats_per_bar.cmp(&right.beats_per_bar))
            .then_with(|| left.phase.cmp(&right.phase))
    });
    hypotheses
}
