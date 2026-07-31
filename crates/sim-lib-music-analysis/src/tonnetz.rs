//! Tonnetz identities, neo-Riemannian actions, and certified graph paths.
//!
//! Transformations act only on [`CanonicalTriad`] identity. Riemannian labels
//! are an explicit projection through [`tonnetz_riemann_label`], so spelling or
//! display changes cannot alter graph topology or transformation identity.

use sim_lib_discrete_graph::{
    Directedness, Graph, GraphError, ShortestPathCertificate, shortest_path, verify_shortest_paths,
};
use sim_lib_pitch_chord::Chord;
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_namer_riemann::label_riemann;
use sim_lib_pitch_set::{BitChord, PitchClassMask};
use thiserror::Error;

/// Major/minor quality carried by a canonical Tonnetz triad.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TriadQuality {
    /// Root-position major identity `{0, 4, 7}`.
    Major,
    /// Root-position minor identity `{0, 3, 7}`.
    Minor,
}

/// Canonical chord identity used by Tonnetz transformations.
///
/// This value deliberately contains no pitch spelling, octave, voicing, or
/// display label. Its pitch-class mask is derived from root and quality.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CanonicalTriad {
    /// Root pitch class.
    pub root: PitchClass,
    /// Major/minor identity.
    pub quality: TriadQuality,
}

impl CanonicalTriad {
    /// Constructs a canonical triad identity.
    pub const fn new(root: PitchClass, quality: TriadQuality) -> Self {
        Self { root, quality }
    }

    /// Returns the rooted pitch-class representation of this identity.
    pub fn bit_chord(self) -> BitChord {
        let third = match self.quality {
            TriadQuality::Major => 4,
            TriadQuality::Minor => 3,
        };
        BitChord {
            mask: PitchClassMask::from_pitch_classes(&[
                self.root,
                self.root.transpose(third),
                self.root.transpose(7),
            ]),
            root: Some(self.root),
        }
    }

    /// Validates and converts a rooted pitch-class chord into canonical identity.
    pub fn from_bit_chord(chord: BitChord) -> Result<Self, TonnetzError> {
        let root = chord.root.ok_or(TonnetzError::MissingRoot)?;
        for quality in [TriadQuality::Major, TriadQuality::Minor] {
            let identity = Self::new(root, quality);
            if identity.bit_chord().mask == chord.mask {
                return Ok(identity);
            }
        }
        Err(TonnetzError::NotCanonicalTriad {
            root,
            mask: chord.mask.bits(),
        })
    }

    /// Applies one neo-Riemannian operation to this identity.
    pub fn apply(self, operation: TonnetzMove) -> Self {
        use TonnetzMove::{LeadingToneExchange, Parallel, Relative};
        use TriadQuality::{Major, Minor};

        match (operation, self.quality) {
            (Parallel, Major) => Self::new(self.root, Minor),
            (Parallel, Minor) => Self::new(self.root, Major),
            (LeadingToneExchange, Major) => Self::new(self.root.transpose(4), Minor),
            (LeadingToneExchange, Minor) => Self::new(self.root.transpose(-4), Major),
            (Relative, Major) => Self::new(self.root.transpose(-3), Minor),
            (Relative, Minor) => Self::new(self.root.transpose(3), Major),
        }
    }

    /// Applies an ordered word in the P/L/R generators.
    ///
    /// The empty word is the identity action. Composition is left-to-right in
    /// slice order, matching the operation order retained by path evidence.
    pub fn apply_moves(self, operations: &[TonnetzMove]) -> Self {
        operations
            .iter()
            .fold(self, |identity, operation| identity.apply(*operation))
    }

    fn graph_index(self) -> usize {
        usize::from(self.root.value()) * 2
            + match self.quality {
                TriadQuality::Major => 0,
                TriadQuality::Minor => 1,
            }
    }
}

/// Neo-Riemannian P/L/R generator.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TonnetzMove {
    /// Parallel transformation (`P`): switch quality while retaining the root.
    Parallel,
    /// Leading-tone exchange (`L`): exchange the root or fifth by semitone.
    LeadingToneExchange,
    /// Relative transformation (`R`): move between relative major and minor.
    Relative,
}

impl TonnetzMove {
    /// Returns the conventional one-letter operation symbol.
    pub const fn symbol(self) -> char {
        match self {
            Self::Parallel => 'P',
            Self::LeadingToneExchange => 'L',
            Self::Relative => 'R',
        }
    }
}

/// One identity-preserving edge in a Tonnetz path.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TonnetzStep {
    /// Identity before the operation.
    pub from: CanonicalTriad,
    /// Applied P/L/R generator.
    pub operation: TonnetzMove,
    /// Identity after the operation.
    pub to: CanonicalTriad,
}

/// Deterministic shortest Tonnetz path with reusable graph evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TonnetzPath {
    /// Canonical identities along the path, including both endpoints.
    pub nodes: Vec<CanonicalTriad>,
    /// Operations between adjacent identities.
    pub steps: Vec<TonnetzStep>,
    /// Unit edge distance returned by the graph solver.
    pub distance: usize,
    /// Verified predecessor-tree certificate from `sim-lib-discrete-graph`.
    pub certificate: ShortestPathCertificate,
}

impl TonnetzPath {
    /// Projects path identities through the existing Riemannian namer.
    ///
    /// Labels are derived output and are never used to recover path operations.
    pub fn riemann_labels(&self) -> Vec<String> {
        self.nodes
            .iter()
            .copied()
            .map(tonnetz_riemann_label)
            .collect()
    }
}

/// Error returned by Tonnetz identity conversion, graph search, or verification.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TonnetzError {
    /// Tonnetz analysis requires an explicit chord root.
    #[error("Tonnetz analysis requires a rooted chord")]
    MissingRoot,
    /// The rooted mask is not the canonical major or minor triad for its root.
    #[error("root {root:?} and pitch-class mask {mask:#05x} are not a canonical triad")]
    NotCanonicalTriad {
        /// Declared chord root.
        root: PitchClass,
        /// Supplied low-twelve pitch-class mask.
        mask: u16,
    },
    /// At least one P/L/R generator must be enabled.
    #[error("Tonnetz analysis requires at least one move")]
    EmptyMoves,
    /// No route exists through the graph induced by the enabled generators.
    #[error("target triad is unreachable through the enabled Tonnetz moves")]
    Unreachable,
    /// The shortest route exists but exceeds the caller's move bound.
    #[error("shortest Tonnetz path needs {required} moves, exceeding limit {limit}")]
    LimitExceeded {
        /// Caller-supplied maximum number of moves.
        limit: usize,
        /// Length of the reproducible shortest route.
        required: usize,
    },
    /// Reusable graph construction, solver, or certificate failure.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// Submitted path evidence does not agree with canonical P/L/R action.
    #[error("invalid Tonnetz path evidence: {0}")]
    InvalidEvidence(String),
}

/// Analyzes a shortest path between two concrete chord values.
///
/// Chords are reduced to rooted pitch-class identity before any transformation;
/// octave, voicing, slash-bass spelling, and display strings are not graph keys.
pub fn analyze_tonnetz(
    from: &Chord,
    to: &Chord,
    moves: &[TonnetzMove],
    limit: usize,
) -> Result<TonnetzPath, TonnetzError> {
    analyze_tonnetz_identities(from.bit_chord(), to.bit_chord(), moves, limit)
}

/// Analyzes a shortest path between rooted pitch-class chord identities.
pub fn analyze_tonnetz_identities(
    from: BitChord,
    to: BitChord,
    moves: &[TonnetzMove],
    limit: usize,
) -> Result<TonnetzPath, TonnetzError> {
    let from = CanonicalTriad::from_bit_chord(from)?;
    let to = CanonicalTriad::from_bit_chord(to)?;
    let moves = normalized_moves(moves)?;
    let graph = tonnetz_graph(&moves)?;
    let shortest = shortest_path(&graph, from.graph_index(), to.graph_index())?;
    let distance = usize::try_from(shortest.distance.ok_or(TonnetzError::Unreachable)?)
        .map_err(|_| TonnetzError::InvalidEvidence("negative unit-edge distance".to_owned()))?;
    if distance > limit {
        return Err(TonnetzError::LimitExceeded {
            limit,
            required: distance,
        });
    }

    let steps = shortest
        .nodes
        .windows(2)
        .map(|pair| {
            let operation = moves
                .iter()
                .copied()
                .find(|operation| pair[0].apply(*operation) == pair[1])
                .ok_or_else(|| {
                    TonnetzError::InvalidEvidence(
                        "shortest-path edge has no enabled P/L/R operation".to_owned(),
                    )
                })?;
            Ok(TonnetzStep {
                from: pair[0],
                operation,
                to: pair[1],
            })
        })
        .collect::<Result<Vec<_>, TonnetzError>>()?;
    let path = TonnetzPath {
        nodes: shortest.nodes,
        steps,
        distance,
        certificate: shortest.certificate,
    };
    verify_tonnetz_path(&path, &moves, limit)?;
    Ok(path)
}

/// Verifies graph and P/L/R action evidence for a Tonnetz path.
pub fn verify_tonnetz_path(
    path: &TonnetzPath,
    moves: &[TonnetzMove],
    limit: usize,
) -> Result<(), TonnetzError> {
    let moves = normalized_moves(moves)?;
    let graph = tonnetz_graph(&moves)?;
    verify_shortest_paths(&graph, &path.certificate)?;
    let Some(from) = path.nodes.first().copied() else {
        return Err(TonnetzError::InvalidEvidence(
            "path has no source".to_owned(),
        ));
    };
    let Some(to) = path.nodes.last().copied() else {
        return Err(TonnetzError::InvalidEvidence(
            "path has no target".to_owned(),
        ));
    };
    if path.certificate.source != from.graph_index() {
        return Err(TonnetzError::InvalidEvidence(
            "certificate source does not match path source".to_owned(),
        ));
    }
    if path.distance != path.steps.len() || path.nodes.len() != path.steps.len() + 1 {
        return Err(TonnetzError::InvalidEvidence(
            "node, step, and distance counts disagree".to_owned(),
        ));
    }
    if path.distance > limit {
        return Err(TonnetzError::LimitExceeded {
            limit,
            required: path.distance,
        });
    }
    for (index, step) in path.steps.iter().enumerate() {
        if step.from != path.nodes[index]
            || step.to != path.nodes[index + 1]
            || !moves.contains(&step.operation)
            || step.from.apply(step.operation) != step.to
        {
            return Err(TonnetzError::InvalidEvidence(format!(
                "step {index} does not match enabled canonical action"
            )));
        }
    }
    let expected = shortest_path(&graph, from.graph_index(), to.graph_index())?;
    if expected.nodes != path.nodes
        || expected.distance != i64::try_from(path.distance).ok()
        || expected.certificate != path.certificate
    {
        return Err(TonnetzError::InvalidEvidence(
            "path differs from deterministic certified shortest path".to_owned(),
        ));
    }
    Ok(())
}

/// Projects a canonical triad through the existing Riemannian naming owner.
pub fn tonnetz_riemann_label(triad: CanonicalTriad) -> String {
    let chord = triad.bit_chord();
    label_riemann(chord.mask, chord.root)
        .expect("canonical major/minor triads always have a Riemannian label")
}

fn normalized_moves(moves: &[TonnetzMove]) -> Result<Vec<TonnetzMove>, TonnetzError> {
    let mut normalized = Vec::new();
    for operation in moves {
        if !normalized.contains(operation) {
            normalized.push(*operation);
        }
    }
    if normalized.is_empty() {
        Err(TonnetzError::EmptyMoves)
    } else {
        Ok(normalized)
    }
}

fn tonnetz_graph(moves: &[TonnetzMove]) -> Result<Graph<CanonicalTriad, i64>, GraphError> {
    let mut nodes = Vec::with_capacity(24);
    for root in 0..12 {
        let root = PitchClass::new(root).expect("Tonnetz roots are valid pitch classes");
        nodes.push(CanonicalTriad::new(root, TriadQuality::Major));
        nodes.push(CanonicalTriad::new(root, TriadQuality::Minor));
    }
    let mut graph = Graph::with_nodes(nodes, Directedness::Directed);
    for source in 0..graph.node_count() {
        let identity = graph.nodes[source];
        for operation in moves {
            graph.add_edge(source, identity.apply(*operation).graph_index(), 1)?;
        }
    }
    Ok(graph)
}
