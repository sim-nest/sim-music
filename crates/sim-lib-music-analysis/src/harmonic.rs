//! Key and chord sequence adapters over generic finite HMM inference.

use sim_lib_numbers_stats::{HiddenMarkovModel, forward_backward, viterbi};
use thiserror::Error;

use crate::{chord_templates, key_templates};

/// One timestamped chroma or feature vector accepted by harmonic decoding.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonicFeatureFrame {
    /// Source-sample timestamp.
    pub at_sample: i64,
    /// Finite non-negative feature weights.
    pub values: Vec<f64>,
}

/// A declared key, chord, or caller-defined harmonic template.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonicTemplate {
    /// Stable human-facing label.
    pub label: String,
    /// Feature-space template weights.
    pub weights: Vec<f64>,
}

impl HarmonicTemplate {
    /// Constructs and validates a non-empty finite non-negative template.
    pub fn new(label: impl Into<String>, weights: Vec<f64>) -> Result<Self, HarmonicDecodeError> {
        let template = Self {
            label: label.into(),
            weights,
        };
        validate_template(&template, 0)?;
        Ok(template)
    }
}

/// State-selection policy over shared posterior and Viterbi evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarmonicDecodeStrategy {
    /// Select each frame's maximum smoothed posterior independently.
    Posterior,
    /// Select the maximum joint-probability state path.
    Viterbi,
}

/// Explicit finite-HMM and result policy for key/chord decoding.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonicDecodePlan {
    /// Selection rule; posterior evidence is retained under both choices.
    pub strategy: HarmonicDecodeStrategy,
    /// Default probability of remaining in one state when transitions are not supplied.
    pub stay_probability: f64,
    /// Probability that a state emits its own nearest-template symbol.
    pub expected_emission_probability: f64,
    /// Maximum posterior alternatives retained per frame.
    pub max_alternatives: usize,
    /// Maximum admitted similarity and HMM work units.
    pub max_work: u64,
}

impl Default for HarmonicDecodePlan {
    fn default() -> Self {
        Self {
            strategy: HarmonicDecodeStrategy::Posterior,
            stay_probability: 0.92,
            expected_emission_probability: 0.90,
            max_alternatives: 8,
            max_work: 10_000_000,
        }
    }
}

/// One posterior alternative retaining both template fit and sequence evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonicAlternative {
    /// Template/state label.
    pub label: String,
    /// Cosine similarity between this frame and the declared template.
    pub similarity: f64,
    /// Smoothed posterior probability after transition evidence.
    pub posterior: f64,
}

/// One selected key or chord frame with confidence and alternatives.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonicFrame {
    /// Source-sample timestamp inherited from input features.
    pub at_sample: i64,
    /// Selected template/state label.
    pub label: String,
    /// Smoothed posterior probability of the selected state.
    pub confidence: f64,
    /// Ranked posterior alternatives, including the selected state.
    pub alternatives: Vec<HarmonicAlternative>,
}

/// Numerical, posterior, path, and deterministic work evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonicSequenceEvidence {
    /// Natural logarithm of the complete observation-sequence likelihood.
    pub log_likelihood: f64,
    /// Count of numerical repairs made by generic normalized inference.
    pub numerical_repairs: u64,
    /// Number of normalized time steps.
    pub normalized_steps: usize,
    /// Viterbi joint log probability when Viterbi selection was requested.
    pub path_log_probability: Option<f64>,
    /// Similarity plus finite-HMM work admitted before execution.
    pub work_used: u64,
    /// Caller-declared work ceiling.
    pub work_limit: u64,
}

/// Decoded harmonic sequence retaining templates, transitions, and evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonicSequence {
    /// Complete selection and resource policy.
    pub plan: HarmonicDecodePlan,
    /// Exact declared or built-in templates used as hidden states.
    pub templates: Vec<HarmonicTemplate>,
    /// Normalized hidden-state transition rows.
    pub transitions: Vec<Vec<f64>>,
    /// Timestamped selected frames and alternatives.
    pub frames: Vec<HarmonicFrame>,
    /// Sequence-level numerical and work evidence.
    pub evidence: HarmonicSequenceEvidence,
}

/// Failure from template adaptation or delegated HMM inference.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum HarmonicDecodeError {
    /// A named input or policy field was invalid.
    #[error("invalid {field}: {reason}")]
    Invalid {
        /// Invalid field.
        field: &'static str,
        /// Stable explanation.
        reason: &'static str,
    },
    /// The declared deterministic work ceiling was insufficient.
    #[error("harmonic decoding needs {required} work units, exceeding {maximum}")]
    WorkLimit {
        /// Preflight work requirement.
        required: u64,
        /// Caller-declared ceiling.
        maximum: u64,
    },
    /// Generic finite-HMM construction or inference failed.
    #[error("harmonic HMM failed: {0}")]
    Hmm(String),
}

/// Decodes caller-declared templates through the generic numbers HMM.
///
/// `transitions` may declare domain-specific rows. When absent, the adapter
/// derives symmetric rows from [`HarmonicDecodePlan::stay_probability`].
pub fn decode_harmonic_sequence(
    frames: &[HarmonicFeatureFrame],
    templates: &[HarmonicTemplate],
    transitions: Option<Vec<Vec<f64>>>,
    plan: &HarmonicDecodePlan,
) -> Result<HarmonicSequence, HarmonicDecodeError> {
    validate_decode(frames, templates, plan)?;
    let states = templates.len();
    let dimensions = frames[0].values.len();
    let work_used = admitted_work(frames.len(), states, dimensions, plan)?;
    let matches = frames
        .iter()
        .map(|frame| {
            templates
                .iter()
                .map(|template| cosine_similarity(&frame.values, &template.weights))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let observations = matches
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .max_by(|(left_index, left), (right_index, right)| {
                    left.total_cmp(right)
                        .then_with(|| right_index.cmp(left_index))
                })
                .map(|(index, _)| index)
                .expect("validated non-empty templates")
        })
        .collect::<Vec<_>>();
    let transitions = transitions.unwrap_or_else(|| default_transitions(states, plan));
    let initial = vec![1.0 / states as f64; states];
    let emissions = confusion_emissions(states, plan.expected_emission_probability);
    let labels = templates
        .iter()
        .map(|template| template.label.clone())
        .collect::<Vec<_>>();
    let model = HiddenMarkovModel::discrete(labels, initial, transitions.clone(), emissions)
        .map_err(|error| HarmonicDecodeError::Hmm(error.to_string()))?;
    let posterior = forward_backward(&model, &observations)
        .map_err(|error| HarmonicDecodeError::Hmm(error.to_string()))?;
    let (selected, path_log_probability) = match plan.strategy {
        HarmonicDecodeStrategy::Posterior => (
            posterior
                .posterior
                .iter()
                .map(|row| maximum_index(row))
                .collect(),
            None,
        ),
        HarmonicDecodeStrategy::Viterbi => {
            let path = viterbi(&model, &observations)
                .map_err(|error| HarmonicDecodeError::Hmm(error.to_string()))?;
            (path.state_indices, Some(path.log_probability))
        }
    };
    let decoded = frames
        .iter()
        .enumerate()
        .map(|(position, frame)| {
            let selected = selected[position];
            let mut alternatives = templates
                .iter()
                .enumerate()
                .map(|(state, template)| HarmonicAlternative {
                    label: template.label.clone(),
                    similarity: matches[position][state],
                    posterior: posterior.posterior[position][state],
                })
                .collect::<Vec<_>>();
            alternatives.sort_by(|left, right| {
                right
                    .posterior
                    .total_cmp(&left.posterior)
                    .then_with(|| right.similarity.total_cmp(&left.similarity))
                    .then_with(|| left.label.cmp(&right.label))
            });
            alternatives.truncate(plan.max_alternatives);
            HarmonicFrame {
                at_sample: frame.at_sample,
                label: templates[selected].label.clone(),
                confidence: posterior.posterior[position][selected],
                alternatives,
            }
        })
        .collect();
    Ok(HarmonicSequence {
        plan: plan.clone(),
        templates: templates.to_vec(),
        transitions,
        frames: decoded,
        evidence: HarmonicSequenceEvidence {
            log_likelihood: posterior.evidence.log_likelihood,
            numerical_repairs: posterior.evidence.numerical_repairs,
            normalized_steps: posterior.evidence.normalized_steps,
            path_log_probability,
            work_used,
            work_limit: plan.max_work,
        },
    })
}

/// Decodes twelve major and twelve minor key profiles with posterior evidence.
pub fn decode_keys(
    frames: &[HarmonicFeatureFrame],
    plan: &HarmonicDecodePlan,
) -> Result<HarmonicSequence, HarmonicDecodeError> {
    decode_harmonic_sequence(frames, &key_templates(), None, plan)
}

/// Decodes twelve major and twelve minor triad templates with HMM evidence.
pub fn decode_chords(
    frames: &[HarmonicFeatureFrame],
    plan: &HarmonicDecodePlan,
) -> Result<HarmonicSequence, HarmonicDecodeError> {
    decode_harmonic_sequence(frames, &chord_templates(), None, plan)
}

fn validate_decode(
    frames: &[HarmonicFeatureFrame],
    templates: &[HarmonicTemplate],
    plan: &HarmonicDecodePlan,
) -> Result<(), HarmonicDecodeError> {
    if frames.is_empty() || templates.is_empty() {
        return Err(invalid_decode(
            "harmonic sequence",
            "at least one frame and template are required",
        ));
    }
    let dimensions = frames[0].values.len();
    if dimensions == 0
        || frames.iter().any(|frame| {
            frame.values.len() != dimensions
                || frame
                    .values
                    .iter()
                    .any(|value| !value.is_finite() || *value < 0.0)
                || frame.values.iter().all(|value| *value <= f64::EPSILON)
        })
    {
        return Err(invalid_decode(
            "harmonic features",
            "equal non-empty finite non-negative and nonzero rows are required",
        ));
    }
    for template in templates {
        validate_template(template, dimensions).map_err(|_| {
            invalid_decode(
                "harmonic templates",
                "labels must be unique and weights must match finite nonzero feature rows",
            )
        })?;
    }
    let mut labels = templates
        .iter()
        .map(|template| template.label.as_str())
        .collect::<Vec<_>>();
    labels.sort_unstable();
    if labels.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid_decode(
            "harmonic templates",
            "template labels must be unique",
        ));
    }
    if !plan.stay_probability.is_finite()
        || !(0.0..=1.0).contains(&plan.stay_probability)
        || !plan.expected_emission_probability.is_finite()
        || !(0.0..=1.0).contains(&plan.expected_emission_probability)
        || plan.max_alternatives == 0
        || plan.max_work == 0
    {
        return Err(invalid_decode(
            "harmonic decode plan",
            "probabilities and positive result/work bounds are required",
        ));
    }
    Ok(())
}

fn validate_template(
    template: &HarmonicTemplate,
    dimensions: usize,
) -> Result<(), HarmonicDecodeError> {
    if template.label.trim().is_empty()
        || template.weights.is_empty()
        || (dimensions != 0 && template.weights.len() != dimensions)
        || template
            .weights
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        || template.weights.iter().all(|value| *value <= f64::EPSILON)
    {
        return Err(invalid_decode(
            "harmonic template",
            "a label and finite non-negative nonzero weights are required",
        ));
    }
    Ok(())
}

fn admitted_work(
    frames: usize,
    states: usize,
    dimensions: usize,
    plan: &HarmonicDecodePlan,
) -> Result<u64, HarmonicDecodeError> {
    let similarity = frames
        .checked_mul(states)
        .and_then(|value| value.checked_mul(dimensions));
    let hmm_passes = if plan.strategy == HarmonicDecodeStrategy::Viterbi {
        4
    } else {
        3
    };
    let hmm = frames
        .checked_mul(states)
        .and_then(|value| value.checked_mul(states))
        .and_then(|value| value.checked_mul(hmm_passes));
    let required = similarity
        .and_then(|similarity| hmm.and_then(|hmm| similarity.checked_add(hmm)))
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(u64::MAX);
    if required > plan.max_work {
        return Err(HarmonicDecodeError::WorkLimit {
            required,
            maximum: plan.max_work,
        });
    }
    Ok(required)
}

fn default_transitions(states: usize, plan: &HarmonicDecodePlan) -> Vec<Vec<f64>> {
    if states == 1 {
        return vec![vec![1.0]];
    }
    let other = (1.0 - plan.stay_probability) / (states - 1) as f64;
    (0..states)
        .map(|from| {
            (0..states)
                .map(|to| {
                    if from == to {
                        plan.stay_probability
                    } else {
                        other
                    }
                })
                .collect()
        })
        .collect()
}

fn confusion_emissions(states: usize, expected: f64) -> Vec<Vec<f64>> {
    if states == 1 {
        return vec![vec![1.0]];
    }
    let other = (1.0 - expected) / (states - 1) as f64;
    (0..states)
        .map(|state| {
            (0..states)
                .map(|symbol| if state == symbol { expected } else { other })
                .collect()
        })
        .collect()
}

fn cosine_similarity(left: &[f64], right: &[f64]) -> f64 {
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    (dot / (left_norm * right_norm).max(f64::EPSILON)).clamp(0.0, 1.0)
}

fn maximum_index(values: &[f64]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|(left_index, left), (right_index, right)| {
            left.total_cmp(right)
                .then_with(|| right_index.cmp(left_index))
        })
        .map(|(index, _)| index)
        .expect("validated non-empty state row")
}

fn invalid_decode(field: &'static str, reason: &'static str) -> HarmonicDecodeError {
    HarmonicDecodeError::Invalid { field, reason }
}
