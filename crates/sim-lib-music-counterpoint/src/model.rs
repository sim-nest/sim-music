use sim_lib_discrete_graph::Graph;
use sim_lib_music_core::{Counterpoint, Melody, ObjectId, Pitch, Time};
use thiserror::Error;

use crate::RuleSet;

/// Exact half-open span `[start, end)` in whole-note units.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeSpan {
    /// Inclusive start.
    pub start: Time,
    /// Exclusive end.
    pub end: Time,
}

impl TimeSpan {
    /// Creates a span. Callers only construct spans from validated music.
    pub fn new(start: Time, end: Time) -> Self {
        Self { start, end }
    }

    /// Returns the exact span duration.
    pub fn duration(&self) -> Time {
        self.end - self.start
    }
}

/// Stable evidence identifying one analyzed voice.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VoiceEvidence {
    /// Zero-based position in the source counterpoint.
    pub index: usize,
    /// Source-derived stable voice identity.
    pub id: ObjectId,
    /// Human-readable source voice name.
    pub name: String,
}

/// Stable evidence identifying one analyzed note and its exact lifetime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEvidence {
    /// Voice containing the note.
    pub voice: VoiceEvidence,
    /// Zero-based note position within the voice.
    pub index: usize,
    /// Stable logical note identity.
    pub note_id: ObjectId,
    /// Stable score-event identity.
    pub event_id: ObjectId,
    /// Exact note lifetime.
    pub span: TimeSpan,
    /// Octave-aware source pitch.
    pub pitch: Pitch,
}

/// A maximal exact interval with an unchanging set of sounding notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlignmentWindow {
    /// Exact half-open window.
    pub span: TimeSpan,
    /// Notes sounding throughout the window, in deterministic voice order.
    pub notes: Vec<NoteEvidence>,
}

/// Direction of one voice between adjacent aligned events.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MotionDirection {
    /// Pitch moved downward.
    Down,
    /// Pitch did not change.
    Static,
    /// Pitch moved upward.
    Up,
}

/// Relative two-voice motion across adjacent exact alignment windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Motion {
    /// Pair of voices, in source order.
    pub voices: [VoiceEvidence; 2],
    /// Previous notes followed by current notes for both voices.
    pub notes: [NoteEvidence; 4],
    /// Exact span from the previous boundary through the current window.
    pub span: TimeSpan,
    /// Motion of the first voice.
    pub first: MotionDirection,
    /// Motion of the second voice.
    pub second: MotionDirection,
    /// Absolute semitone interval before the motion.
    pub interval_before: i32,
    /// Absolute semitone interval after the motion.
    pub interval_after: i32,
}

/// Concrete metric evidence attached to a rule outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricEvidence {
    /// Stable metric name.
    pub metric: String,
    /// Observed exact or integer value.
    pub observed: String,
    /// Declared limit or accepted set.
    pub expected: String,
    /// Unit or comparison domain.
    pub unit: String,
    /// Additional inspectable facts used by the decision.
    pub facts: Vec<String>,
}

/// One failed rule with complete source and measurement evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// Stable rule identifier.
    pub rule: String,
    /// Human-readable rule explanation.
    pub message: String,
    /// Every involved voice.
    pub voices: Vec<VoiceEvidence>,
    /// Every involved note.
    pub notes: Vec<NoteEvidence>,
    /// Exact affected span.
    pub span: TimeSpan,
    /// Measurement proving why the rule failed.
    pub metric: MetricEvidence,
}

/// Provenance distinguishing inspection of source material from generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisProvenance {
    /// Stable analysis mode; always `"existing-counterpoint"`.
    pub mode: String,
    /// Rule-set identifier.
    pub rule_set: String,
    /// Exact conversion and alignment facts.
    pub facts: Vec<String>,
}

/// Complete analysis of existing counterpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterpointReport {
    /// Exact event-boundary alignment.
    pub alignment: Vec<AlignmentWindow>,
    /// All adjacent two-voice motions.
    pub motions: Vec<Motion>,
    /// Every rule violation, each with its own span and metric evidence.
    pub violations: Vec<Violation>,
    /// Source and policy provenance.
    pub provenance: AnalysisProvenance,
}

impl CounterpointReport {
    /// Returns `true` when no declared rule failed.
    pub fn is_legal(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Result of fusing analyzed stretto entries into a viewable counterpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoFusion {
    /// Materialized voices ordered by onset and stable entry id.
    pub counterpoint: Counterpoint,
    /// Entry ids represented by the fused value.
    pub entry_ids: Vec<usize>,
    /// Explicit statement that this is a derived analysis view.
    pub mode: String,
    /// Transform-owner provenance for every materialized entry.
    pub provenance: Vec<String>,
}

/// Contrapuntal form delegated to the music transform owner.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ContrapuntalForm {
    /// Preserve pitch and time order.
    Original,
    /// Reverse exact note placement in time.
    Retrograde,
    /// Invert pitch around the supplied axis.
    Inversion {
        /// Pitch inversion axis.
        axis: Pitch,
    },
    /// Apply pitch inversion followed by retrograde.
    RetrogradeInversion {
        /// Pitch inversion axis.
        axis: Pitch,
    },
}

/// One reusable transform request for a stretto entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoTransform {
    /// Contrapuntal pitch/time form.
    pub form: ContrapuntalForm,
    /// Chromatic transposition applied after the form.
    pub transposition: i32,
    /// Positive exact duration factor.
    pub duration_factor: Time,
}

impl StrettoTransform {
    /// Original form at one chromatic transposition.
    pub fn original(transposition: i32) -> Self {
        Self {
            form: ContrapuntalForm::Original,
            transposition,
            duration_factor: Time::from_integer(1),
        }
    }
}

/// Bounded policy for deriving and comparing stretto entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoPolicy {
    /// Exact follower delays; the anchor at zero is supplied automatically.
    pub delays: Vec<Time>,
    /// Transform requests crossed with every admitted delay.
    pub transforms: Vec<StrettoTransform>,
    /// Minimum temporal intersection for any compatible pair.
    pub minimum_overlap: Time,
    /// Counterpoint rules used for every pairwise compatibility decision.
    pub compatibility_rules: RuleSet,
    /// Maximum graph nodes, including the anchor.
    pub max_entries: usize,
    /// Minimum voices in a reported maximal clique.
    pub minimum_cluster_voices: usize,
    /// Maximum reported maximal cliques.
    pub max_clusters: usize,
    /// Maximum clusters in one reported simple chain.
    pub max_chain_length: usize,
}

impl Default for StrettoPolicy {
    fn default() -> Self {
        let mut compatibility_rules = RuleSet::open();
        compatibility_rules.id = "stretto-default".to_owned();
        compatibility_rules.intervals.consonant_harmonic_classes = vec![0, 3, 4, 5];
        Self {
            delays: vec![Time::new(1, 4), Time::new(1, 2), Time::new(3, 4)],
            transforms: (0..12).map(StrettoTransform::original).collect(),
            minimum_overlap: Time::new(1, 4),
            compatibility_rules,
            max_entries: 64,
            minimum_cluster_voices: 3,
            max_clusters: 128,
            max_chain_length: 8,
        }
    }
}

/// One materialized graph node derived from a subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoEntry {
    /// Stable graph-local id.
    pub id: usize,
    /// Exact onset relative to the anchor.
    pub delay: Time,
    /// Transform request and provenance.
    pub transform: StrettoTransform,
    /// Materialized melody returned through transform-owner operations.
    pub melody: Melody,
}

/// Exact overlap facts for one entry pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlapEvidence {
    /// Temporal intersection of both entry extents.
    pub span: TimeSpan,
    /// Exact alignment windows in which both voices sound.
    pub simultaneous_windows: usize,
    /// Histogram of observed harmonic interval classes.
    pub interval_classes: Vec<(u8, usize)>,
    /// Facts naming the policy and pairwise analysis path.
    pub facts: Vec<String>,
}

/// Weight carried by a compatible edge in the shared graph value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoCompatibility {
    /// Exact pairwise overlap evidence.
    pub overlap: OverlapEvidence,
    /// Number of pairwise rule violations; always zero for a graph edge.
    pub violation_count: usize,
}

/// One compatible pair represented by a compatibility-graph edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoCouple {
    /// First graph-node id.
    pub leader: usize,
    /// Second graph-node id.
    pub follower: usize,
    /// Exact pairwise evidence.
    pub compatibility: StrettoCompatibility,
}

/// One rejected pair retained outside the compatibility graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoRejection {
    /// First graph-node id.
    pub first: usize,
    /// Second graph-node id.
    pub second: usize,
    /// Exact temporal and interval evidence.
    pub overlap: OverlapEvidence,
    /// Per-rule reasons for rejection.
    pub violations: Vec<Violation>,
}

/// A maximal pairwise-compatible clique.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoCluster {
    /// Graph-node ids in deterministic order.
    pub entries: Vec<usize>,
    /// Compatibility edge ids proving every pair.
    pub edge_ids: Vec<usize>,
    /// Fused analysis view.
    pub fusion: StrettoFusion,
}

/// A sequence of clusters joined by normalized suffix/prefix overlap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrettoChain {
    /// Cluster indices in traversal order.
    pub clusters: Vec<usize>,
    /// Entry overlap at each adjacent join.
    pub overlaps: Vec<usize>,
    /// Normalized entry ids after splicing the chain.
    pub fused_entries: Vec<usize>,
}

/// Complete bounded stretto compatibility result.
#[derive(Clone, Debug, PartialEq)]
pub struct StrettoGraph {
    /// Shared undirected graph whose node labels are materialized entries.
    pub compatibility: Graph<StrettoEntry, StrettoCompatibility>,
    /// Every compatible graph edge as a named couple.
    pub couples: Vec<StrettoCouple>,
    /// Rejected candidate pairs with rule evidence.
    pub rejections: Vec<StrettoRejection>,
    /// Coarse connected components from the shared graph owner.
    pub components: Vec<Vec<usize>>,
    /// Maximal pairwise-compatible cliques.
    pub clusters: Vec<StrettoCluster>,
    /// Directed cluster-overlap graph; edge weights are overlap lengths.
    pub chain_graph: Graph<usize, usize>,
    /// Longest bounded simple cluster chains.
    pub chains: Vec<StrettoChain>,
    /// Explicit analysis provenance.
    pub provenance: Vec<String>,
}

/// Failure to validate or materialize a stretto analysis.
#[derive(Debug, Error)]
pub enum StrettoError {
    /// Policy contains an invalid bound or exact time.
    #[error("invalid stretto policy: {0}")]
    InvalidPolicy(String),
    /// A delegated music transform failed.
    #[error(transparent)]
    Transform(#[from] sim_lib_music_transform::TransformError),
    /// A transformed line could not be represented as monophonic melody.
    #[error("transformed stretto entry is not monophonic: {0}")]
    NonMonophonic(String),
    /// A graph operation failed.
    #[error(transparent)]
    Graph(#[from] sim_lib_discrete_graph::GraphError),
    /// Fused counterpoint construction failed.
    #[error(transparent)]
    Music(#[from] sim_lib_music_core::MusicError),
}
