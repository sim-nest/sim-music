//! Public harmonization request, strategy, result, and receipt values.

use sim_lib_discrete_graph::{AlgorithmReceipt, LayeredCertificate};
use sim_lib_discrete_search::{SearchReceipt, SearchStatus};
use sim_lib_pitch_set::PitchClassMask;

use crate::{ChordPalette, ChordTemplate, HarmonyError, HarmonyEvaluation, HarmonyRuleSet};

/// Fixed precision used when ordering finite declarative scores.
pub const HARMONY_SCORE_SCALE: i64 = 1_000_000;

/// Caller declaration for the optimistic estimate used by beam search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyHeuristic {
    /// Stable policy name retained in the receipt.
    pub id: String,
    /// Declared lower bound for each still-unfilled melody position.
    pub lower_bound_per_remaining_micros: i64,
    /// Whether the caller warrants that the declared bound is admissible.
    pub admissible: bool,
}

impl HarmonyHeuristic {
    /// Declares the zero heuristic, admissible when remaining scores cannot be negative.
    pub fn zero(id: impl Into<String>, admissible: bool) -> Self {
        Self {
            id: id.into(),
            lower_bound_per_remaining_micros: 0,
            admissible,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), HarmonyError> {
        if self.id.is_empty()
            || self
                .id
                .chars()
                .any(|character| !character.is_ascii() || character.is_whitespace())
        {
            return Err(HarmonyError::InvalidId(self.id.clone()));
        }
        Ok(())
    }
}

/// Search policy applied to one shared harmonization problem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HarmonizationStrategy {
    /// Deterministic depth-first enumeration with rule checks after each choice.
    RecursiveExhaustive,
    /// Depth-first backtracking that factors legal choices before committing them.
    FactoredBacktracking,
    /// Exact global shortest path over complete legal prefix layers.
    LayeredDp,
    /// Width-bounded global search ordered by accumulated score plus a declared estimate.
    Beam {
        /// Maximum retained frontier width.
        width: usize,
        /// Explicit optimistic or heuristic remaining-score declaration.
        heuristic: HarmonyHeuristic,
    },
}

impl HarmonizationStrategy {
    /// Stable strategy name used by diagnostics and recipes.
    pub fn label(&self) -> &'static str {
        match self {
            Self::RecursiveExhaustive => "recursive-exhaustive",
            Self::FactoredBacktracking => "factored-backtracking",
            Self::LayeredDp => "layered-dp",
            Self::Beam { .. } => "beam",
        }
    }
}

/// Melody, chord vocabulary, and declarative rules for one planner run.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonizationRequest {
    /// Required pitch-class sets in phrase order.
    pub melody: Vec<PitchClassMask>,
    /// Deterministically ordered candidate chords.
    pub palette: ChordPalette,
    /// Hard legality and soft score definitions.
    pub rules: HarmonyRuleSet,
}

impl HarmonizationRequest {
    /// Validates the finite problem before any bounded work begins.
    pub fn validate(&self) -> Result<(), HarmonyError> {
        if self.melody.is_empty() {
            return Err(HarmonyError::Empty("harmonization melody"));
        }
        self.palette.validate()?;
        self.rules.validate()
    }
}

/// One complete or explicitly partial harmonization candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonizationResult {
    /// Chords selected in melody order.
    pub progression: Vec<ChordTemplate>,
    /// Palette indices selected in deterministic order.
    pub palette_indices: Vec<usize>,
    /// Per-position hard and soft evidence.
    pub evaluations: Vec<HarmonyEvaluation>,
    /// Quantized accumulated minimization score.
    pub score_micros: i64,
    /// False only for a prefix returned when a global bound stopped layering.
    pub complete: bool,
}

/// Aggregated inspectable hard-rule rejection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyRejection {
    /// Declarative hard-rule identity.
    pub rule_id: String,
    /// Stable facts from the failed rule evaluation.
    pub facts: Vec<String>,
    /// Number of candidates rejected with these exact facts.
    pub count: u64,
}

/// Certified optimum evidence returned by layered dynamic programming.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonyOptimalityEvidence {
    /// Exact quantized score certified by Bellman backpointers.
    pub total_score_micros: i64,
    /// Stable selected cell index in each prefix layer, including the root layer.
    pub layer_indices: Vec<usize>,
    /// Checkable Bellman table and stable predecessor indices.
    pub certificate: LayeredCertificate<i64>,
}

/// Unified receipt for local search and global layered planning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarmonizationReceipt {
    /// Strategy selected by the caller.
    pub strategy: HarmonizationStrategy,
    /// Honest complete, partial, cancelled, or infeasible outcome.
    pub status: SearchStatus,
    /// Stable stop reason for bounded or invalid global runs.
    pub reason: Option<String>,
    /// Total charged generic-search or layered work.
    pub work_used: u64,
    /// Number of musical candidates actually evaluated.
    pub evaluated_candidates: u64,
    /// Number of returned complete or partial candidates.
    pub result_count: usize,
    /// True only when completion proves no lower-score result exists.
    pub optimal: bool,
    /// Generic bounded-search evidence for recursive, factored, and beam runs.
    pub search: Option<SearchReceipt>,
    /// Generic graph-algorithm evidence for a completed layered run.
    pub layered: Option<AlgorithmReceipt>,
    /// Bellman certificate for a completed layered optimum.
    pub optimality: Option<HarmonyOptimalityEvidence>,
    /// Bounded aggregate of every distinct failed hard-rule decision.
    pub rejections: Vec<HarmonyRejection>,
}

/// Results and evidence produced by one harmonization run.
#[derive(Clone, Debug, PartialEq)]
pub struct HarmonizationRun {
    /// Candidates sorted by score, then deterministic palette-index path.
    pub results: Vec<HarmonizationResult>,
    /// Bounds, strategy, failures, cost, and optimality evidence.
    pub receipt: HarmonizationReceipt,
}
