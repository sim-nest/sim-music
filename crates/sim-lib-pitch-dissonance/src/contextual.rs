use sim_lib_pitch_core::Pitch;
use sim_lib_pitch_ratio::RatioPolicy;

use crate::contextual_support::{
    apply_duplicate_policy, contextual_interval_vector, contextual_sonance, finite_or_zero,
    interval_roughness, normalize_contextual, pair_count, pitch_multiset, pseudo_partial_cost,
    ratio_cost, voice_pairs, weighted_score,
};
use crate::model::merge_contributions;
use crate::{IntervalMergeMode, Sonance};

/// A voiced pitch event used by contextual sonance comparison.
///
/// Unlike [`sim_lib_pitch_set::PitchClassMask`], this keeps octave, amplitude,
/// input order, and optional voice identity. Duplicate notes remain separate
/// events unless the caller explicitly selects [`DuplicatePolicy::Collapse`].
#[derive(Clone, Debug, PartialEq)]
pub struct ContextualPitch {
    /// Stable input identity retained in comparison reports.
    pub id: String,
    /// Optional voice identity used by leading and continuity models.
    pub voice: Option<String>,
    /// Octave-aware pitch.
    pub pitch: Pitch,
    /// Non-negative event amplitude.
    pub amplitude: f64,
}

impl ContextualPitch {
    /// Builds a pitch event with amplitude `1.0` and no voice identity.
    pub fn unvoiced(id: impl Into<String>, pitch: Pitch) -> Self {
        Self {
            id: id.into(),
            voice: None,
            pitch,
            amplitude: 1.0,
        }
    }

    pub(crate) fn weight(&self) -> f64 {
        if self.amplitude.is_finite() {
            self.amplitude.max(0.0)
        } else {
            0.0
        }
    }
}

/// Duplicate-note handling for contextual comparison.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DuplicatePolicy {
    /// Retain multiplicity as separate note events.
    Retain,
    /// Collapse equal pitch events inside each side before scoring.
    Collapse,
}

impl DuplicatePolicy {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Retain => "retain",
            Self::Collapse => "collapse",
        }
    }
}

/// Normalization policy for contextual sonance components.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SonanceNormalization {
    /// Keep raw accumulated mass.
    Raw,
    /// Normalize by the relevant pair or voice-pair opportunity.
    PerPair,
}

impl SonanceNormalization {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::PerPair => "per-pair",
        }
    }
}

/// Voice identity policy for comparing one context window to another.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VoiceIdentityPolicy {
    /// Pair notes by input index.
    ByIndex,
    /// Pair notes by matching non-empty voice ids, falling back to input index.
    ByVoiceThenIndex,
}

impl VoiceIdentityPolicy {
    fn name(self) -> &'static str {
        match self {
            Self::ByIndex => "by-index",
            Self::ByVoiceThenIndex => "by-voice-then-index",
        }
    }
}

/// Context window around a sonance comparison.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ContextWindow {
    /// Number of preceding events considered part of continuity context.
    pub before: usize,
    /// Number of following events considered part of continuity context.
    pub after: usize,
}

impl ContextWindow {
    /// Returns the local two-chord comparison window.
    pub const fn local() -> Self {
        Self {
            before: 0,
            after: 0,
        }
    }
}

/// Component weights used by contextual sonance models.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ContextualSonanceWeights {
    /// Weight for roughness-like mass.
    pub roughness: f64,
    /// Weight for normalized-density contributions.
    pub density: f64,
    /// Weight for harmonic-context contributions.
    pub harmonic_context: f64,
}

impl Default for ContextualSonanceWeights {
    fn default() -> Self {
        Self {
            roughness: 1.0,
            density: 1.0,
            harmonic_context: 1.0,
        }
    }
}

/// Typed configuration for contextual sonance comparison.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ContextualSonanceOptions {
    /// Temporal/event window to include in provenance and continuity scoring.
    pub window: ContextWindow,
    /// Duplicate-note policy.
    pub duplicates: DuplicatePolicy,
    /// Pair normalization policy.
    pub normalization: SonanceNormalization,
    /// Merge policy used by pairwise component models.
    pub merge: IntervalMergeMode,
    /// Voice pairing policy.
    pub voice_identity: VoiceIdentityPolicy,
    /// Component weighting policy.
    pub weights: ContextualSonanceWeights,
    /// Exact-ratio admissibility policy used by the ratio model.
    pub ratio_policy: RatioPolicy,
}

impl ContextualSonanceOptions {
    /// Returns the default contextual policy, retaining duplicate events and
    /// normalizing pair components per opportunity.
    pub fn standard() -> Self {
        Self {
            window: ContextWindow::local(),
            duplicates: DuplicatePolicy::Retain,
            normalization: SonanceNormalization::PerPair,
            merge: IntervalMergeMode::SumPairs,
            voice_identity: VoiceIdentityPolicy::ByVoiceThenIndex,
            weights: ContextualSonanceWeights::default(),
            ratio_policy: RatioPolicy::default(),
        }
    }
}

/// Input identity retained for one side of a contextual report.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextualInputIdentity {
    /// Event ids in input order.
    pub ids: Vec<String>,
    /// Voice ids in input order.
    pub voices: Vec<Option<String>>,
    /// Absolute semitone values in input order.
    pub semitones: Vec<i32>,
    /// Amplitudes in input order.
    pub amplitudes: Vec<f64>,
}

impl ContextualInputIdentity {
    fn from_notes(notes: &[ContextualPitch]) -> Self {
        Self {
            ids: notes.iter().map(|note| note.id.clone()).collect(),
            voices: notes.iter().map(|note| note.voice.clone()).collect(),
            semitones: notes.iter().map(|note| note.pitch.semitone()).collect(),
            amplitudes: notes.iter().map(|note| note.amplitude).collect(),
        }
    }
}

/// The output of one contextual sonance model.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextualSonanceComponent {
    /// Stable model name.
    pub model: &'static str,
    /// Weighted compatibility projection for this component.
    pub score: f64,
    /// Typed sonance components.
    pub sonance: Sonance,
}

/// Combined contextual sonance comparison report.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextualSonanceReport {
    /// Source chord/window identity.
    pub from: ContextualInputIdentity,
    /// Target chord/window identity.
    pub to: ContextualInputIdentity,
    /// Effective typed policy.
    pub options: ContextualSonanceOptions,
    /// One retained component per requested model.
    pub components: Vec<ContextualSonanceComponent>,
}

impl ContextualSonanceReport {
    /// Returns the sum of component scores without dropping component identity.
    pub fn total_score(&self) -> f64 {
        self.components
            .iter()
            .map(|component| component.score)
            .sum()
    }
}

/// A pluggable model for comparing two contextual pitch windows.
pub trait ContextualSonanceModel {
    /// Stable model name.
    fn name(&self) -> &'static str;

    /// Compare `from` and `to` using explicit contextual options.
    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance;
}

/// Registry of contextual sonance models.
#[derive(Default)]
pub struct ContextualSonanceRegistry {
    models: Vec<Box<dyn ContextualSonanceModel>>,
}

impl ContextualSonanceRegistry {
    /// Builds a registry populated with every built-in contextual sonance model.
    pub fn new_with_builtins() -> Self {
        Self {
            models: vec![
                Box::new(ContextualRoughnessModel),
                Box::new(CommonalityModel),
                Box::new(LeadingModel),
                Box::new(MotionModel),
                Box::new(PseudoPartialModel),
                Box::new(ContextualIntervalVectorModel),
                Box::new(ExperimentalRatioModel),
            ],
        }
    }

    /// Returns sorted model names.
    pub fn list(&self) -> Vec<&'static str> {
        let mut names = self
            .models
            .iter()
            .map(|model| model.name())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    /// Compare two windows with every registered model.
    pub fn compare_all(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> ContextualSonanceReport {
        self.compare_named(self.list().as_slice(), from, to, options)
    }

    /// Compare two windows with the named model subset, preserving request order.
    pub fn compare_named(
        &self,
        models: &[&str],
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> ContextualSonanceReport {
        let from_identity = ContextualInputIdentity::from_notes(from);
        let to_identity = ContextualInputIdentity::from_notes(to);
        let from_notes = apply_duplicate_policy(from, options.duplicates);
        let to_notes = apply_duplicate_policy(to, options.duplicates);
        let components = models
            .iter()
            .filter_map(|name| {
                self.models
                    .iter()
                    .find(|model| model.name() == *name)
                    .map(|model| {
                        let sonance = model.compare(&from_notes, &to_notes, options);
                        ContextualSonanceComponent {
                            model: model.name(),
                            score: weighted_score(&sonance, options.weights),
                            sonance,
                        }
                    })
            })
            .collect();
        ContextualSonanceReport {
            from: from_identity,
            to: to_identity,
            options,
            components,
        }
    }
}

/// Roughness over simultaneous interval opportunities before and after motion.
pub struct ContextualRoughnessModel;

/// Note commonality across source and target windows.
pub struct CommonalityModel;

/// Voice-leading displacement model.
pub struct LeadingModel;

/// Directional motion-continuity model.
pub struct MotionModel;

/// Pseudo-partial relation model for non-spectral pitch input.
pub struct PseudoPartialModel;

/// Multiplicity-aware interval-vector delta model.
pub struct ContextualIntervalVectorModel;

/// Exact-ratio experimental contextual model.
pub struct ExperimentalRatioModel;

impl ContextualSonanceModel for ContextualRoughnessModel {
    fn name(&self) -> &'static str {
        "roughness"
    }

    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance {
        let from_mass = interval_roughness(from, options);
        let to_mass = interval_roughness(to, options);
        contextual_sonance(
            self.name(),
            from_mass + to_mass,
            normalize_contextual(from_mass + to_mass, from.len() + to.len(), options),
            (to_mass - from_mass).abs(),
            options,
            vec![
                format!("from-pairs={}", pair_count(from.len())),
                format!("to-pairs={}", pair_count(to.len())),
            ],
        )
    }
}

impl ContextualSonanceModel for CommonalityModel {
    fn name(&self) -> &'static str {
        "commonality"
    }

    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance {
        let from_counts = pitch_multiset(from);
        let to_counts = pitch_multiset(to);
        let retained = from_counts
            .iter()
            .map(|(pitch, count)| count.min(to_counts.get(pitch).unwrap_or(&0)))
            .sum::<usize>();
        let total = from.len().max(to.len()).max(1);
        let changed = total.saturating_sub(retained);
        contextual_sonance(
            self.name(),
            changed as f64,
            changed as f64 / total as f64,
            retained as f64 / total as f64,
            options,
            vec![
                format!("retained-events={retained}"),
                format!("duplicate-policy={}", options.duplicates.name()),
            ],
        )
    }
}

impl ContextualSonanceModel for LeadingModel {
    fn name(&self) -> &'static str {
        "leading"
    }

    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance {
        let pairs = voice_pairs(from, to, options.voice_identity);
        let motions = pairs
            .iter()
            .map(|(left, right)| (right.pitch.semitone() - left.pitch.semitone()).abs() as f64)
            .collect::<Vec<_>>();
        let roughness = merge_contributions(&motions, options.merge);
        contextual_sonance(
            self.name(),
            roughness,
            if pairs.is_empty() {
                0.0
            } else {
                roughness / (pairs.len() as f64 * 12.0)
            },
            pairs.len() as f64,
            options,
            vec![
                format!("paired-voices={}", pairs.len()),
                format!("voice-policy={}", options.voice_identity.name()),
            ],
        )
    }
}

impl ContextualSonanceModel for MotionModel {
    fn name(&self) -> &'static str {
        "motion"
    }

    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance {
        let deltas = voice_pairs(from, to, options.voice_identity)
            .into_iter()
            .map(|(left, right)| right.pitch.semitone() - left.pitch.semitone())
            .collect::<Vec<_>>();
        let mut parallel = 0usize;
        let mut contrary = 0usize;
        let mut oblique = 0usize;
        for (index, left) in deltas.iter().enumerate() {
            for right in deltas.iter().skip(index + 1) {
                match (left.signum(), right.signum()) {
                    (0, _) | (_, 0) => oblique += 1,
                    (a, b) if a == b => parallel += 1,
                    _ => contrary += 1,
                }
            }
        }
        let opportunities = pair_count(deltas.len()).max(1);
        contextual_sonance(
            self.name(),
            parallel as f64,
            parallel as f64 / opportunities as f64,
            (contrary + oblique) as f64 / opportunities as f64,
            options,
            vec![
                format!("parallel={parallel}"),
                format!("contrary={contrary}"),
                format!("oblique={oblique}"),
            ],
        )
    }
}

impl ContextualSonanceModel for PseudoPartialModel {
    fn name(&self) -> &'static str {
        "pseudo-partial"
    }

    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance {
        let from_cost = pseudo_partial_cost(from);
        let to_cost = pseudo_partial_cost(to);
        contextual_sonance(
            self.name(),
            from_cost + to_cost,
            normalize_contextual(from_cost + to_cost, from.len() + to.len(), options),
            (to_cost - from_cost).abs(),
            options,
            vec!["partials=1..8".to_owned()],
        )
    }
}

impl ContextualSonanceModel for ContextualIntervalVectorModel {
    fn name(&self) -> &'static str {
        "interval-vector"
    }

    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance {
        let from_vector = contextual_interval_vector(from);
        let to_vector = contextual_interval_vector(to);
        let delta = from_vector
            .iter()
            .zip(to_vector)
            .map(|(left, right)| left.abs_diff(right) as f64)
            .collect::<Vec<_>>();
        let roughness = merge_contributions(&delta, options.merge);
        contextual_sonance(
            self.name(),
            roughness,
            normalize_contextual(
                roughness,
                pair_count(from.len()).max(pair_count(to.len())),
                options,
            ),
            0.0,
            options,
            vec![
                format!("from-vector={from_vector:?}"),
                format!("to-vector={to_vector:?}"),
            ],
        )
    }
}

impl ContextualSonanceModel for ExperimentalRatioModel {
    fn name(&self) -> &'static str {
        "ratio"
    }

    fn compare(
        &self,
        from: &[ContextualPitch],
        to: &[ContextualPitch],
        options: ContextualSonanceOptions,
    ) -> Sonance {
        let from_cost = ratio_cost(from, options.ratio_policy);
        let to_cost = ratio_cost(to, options.ratio_policy);
        let rejected = usize::from(!from_cost.is_finite()) + usize::from(!to_cost.is_finite());
        let roughness = finite_or_zero(from_cost) + finite_or_zero(to_cost);
        contextual_sonance(
            self.name(),
            roughness,
            normalize_contextual(roughness, from.len() + to.len(), options),
            (finite_or_zero(to_cost) - finite_or_zero(from_cost)).abs(),
            options,
            vec![
                format!("ratio-policy={:?}", options.ratio_policy),
                format!("rejected-ratio-windows={rejected}"),
                "dialect=experimental-ratio".to_owned(),
            ],
        )
    }
}
