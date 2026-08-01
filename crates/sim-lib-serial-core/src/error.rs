//! Typed construction and validation failures.

use crate::{AggregateRuleKind, AlphabetId, ProjectionId};
use sim_lib_discrete_rank::RankAdapterError;
use thiserror::Error;

/// Failure while defining an alphabet or registering its stable identity.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum AlphabetError {
    /// An alphabet id was empty or did not use the stable portable syntax.
    #[error("invalid alphabet id {value:?}: {reason}")]
    InvalidId {
        /// Rejected id text.
        value: String,
        /// Specific syntax violation.
        reason: &'static str,
    },
    /// A finite musical alphabet contained no symbols.
    #[error("alphabet {id} must contain at least one symbol")]
    Empty {
        /// Stable id of the empty alphabet.
        id: AlphabetId,
    },
    /// The same symbol occupied two canonical alphabet positions.
    #[error("alphabet {id} repeats positions {first} and {duplicate}")]
    DuplicateSymbol {
        /// Stable alphabet id.
        id: AlphabetId,
        /// First canonical position.
        first: usize,
        /// Repeated canonical position.
        duplicate: usize,
    },
    /// A registry already contained the stable id being inserted.
    #[error("duplicate alphabet id {0}")]
    DuplicateId(AlphabetId),
}

/// Failure while compiling symbolic aggregate-rule data for one alphabet.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum AggregateRuleError {
    /// The alphabet itself was invalid.
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    /// A declaration named a symbol outside the alphabet.
    #[error("aggregate declaration contains a foreign symbol for alphabet {alphabet_id}")]
    ForeignSymbol {
        /// Stable alphabet id.
        alphabet_id: AlphabetId,
    },
    /// The same symbol was declared more than once.
    #[error("aggregate declaration repeats alphabet position {position}")]
    DuplicateDeclaration {
        /// Canonical alphabet position identified from the supplied symbol.
        position: usize,
    },
    /// A multiplicity declaration omitted an alphabet symbol.
    #[error("aggregate declaration is missing alphabet position {position}")]
    MissingDeclaration {
        /// Canonical alphabet position without a declaration.
        position: usize,
    },
    /// A multiplicity was zero where the selected rule requires presence.
    #[error("aggregate declaration gives zero multiplicity at alphabet position {position}")]
    ZeroMultiplicity {
        /// Canonical alphabet position with zero multiplicity.
        position: usize,
    },
    /// An omission rule named no omitted symbols.
    #[error("declared-omissions rule must omit at least one symbol")]
    NoOmissions,
    /// A rule omitted every symbol and could not admit a musical series.
    #[error("aggregate rule omits every symbol in alphabet {0}")]
    OmitsEverything(AlphabetId),
    /// Projected class identity did not use the stable portable syntax.
    #[error("invalid projection id {value:?}: {reason}")]
    InvalidProjectionId {
        /// Rejected projection id text.
        value: String,
        /// Specific syntax violation.
        reason: &'static str,
    },
    /// Two projected classes reused the same stable id.
    #[error("duplicate projected class id {0}")]
    DuplicateProjectionId(ProjectionId),
    /// A projected class had no source symbols.
    #[error("projected class {0} must contain at least one symbol")]
    EmptyProjectionClass(ProjectionId),
    /// An alphabet symbol was assigned to more than one projected class.
    #[error("alphabet position {position} belongs to multiple projected classes")]
    DuplicateProjectionMember {
        /// Canonical alphabet position assigned twice.
        position: usize,
    },
    /// An alphabet symbol was not assigned to a projected class.
    #[error("alphabet position {position} has no projected class")]
    MissingProjectionMember {
        /// Canonical alphabet position without a class.
        position: usize,
    },
    /// Declared multiplicities overflowed the platform series length.
    #[error("aggregate multiplicity total exceeds the supported series length")]
    MultiplicityOverflow,
    /// The rule's declarations were compiled for another alphabet identity.
    #[error("aggregate rule belongs to {rule_id}, not {series_id}")]
    AlphabetMismatch {
        /// Alphabet id retained by the rule.
        rule_id: AlphabetId,
        /// Alphabet id supplied to series construction.
        series_id: AlphabetId,
    },
    /// The rule's declarations were compiled for another alphabet cardinality.
    #[error("aggregate rule expects alphabet size {expected}, got {found}")]
    CardinalityMismatch {
        /// Cardinality retained by the rule.
        expected: usize,
        /// Cardinality supplied to series construction.
        found: usize,
    },
}

/// Failure while validating or ranking a symbol-bearing series.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SeriesError {
    /// The supplied alphabet was invalid.
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    /// The aggregate rule was invalid for the supplied alphabet.
    #[error(transparent)]
    Rule(#[from] AggregateRuleError),
    /// A series position contained a symbol outside the alphabet.
    #[error("series position {position} is foreign to alphabet {alphabet_id}")]
    ForeignSymbol {
        /// Position in the supplied series order.
        position: usize,
        /// Stable alphabet id.
        alphabet_id: AlphabetId,
    },
    /// A no-repeat rule encountered a repeated symbol.
    #[error("series position {position} repeats position {first}")]
    RepeatedSymbol {
        /// Position of the repeated occurrence.
        position: usize,
        /// Position of the first occurrence.
        first: usize,
    },
    /// Series length differed from the rule's declared total.
    #[error("aggregate rule expects {expected} symbols, got {found}")]
    WrongLength {
        /// Required number of series positions.
        expected: usize,
        /// Supplied number of series positions.
        found: usize,
    },
    /// An alphabet symbol occurred a different number of times than declared.
    #[error("alphabet position {alphabet_position} occurs {found} times; expected {expected}")]
    MultiplicityMismatch {
        /// Canonical position of the affected symbol.
        alphabet_position: usize,
        /// Required number of occurrences.
        expected: usize,
        /// Observed number of occurrences.
        found: usize,
    },
    /// A projected class occurred a different number of times than declared.
    #[error("projected class {class_id} occurs {found} times; expected {expected}")]
    ProjectionMismatch {
        /// Stable projected class id.
        class_id: ProjectionId,
        /// Required number of occurrences.
        expected: usize,
        /// Observed number of occurrences.
        found: usize,
    },
    /// The valid series is not an exactly-once permutation of its alphabet.
    #[error("series is not an exactly-once permutation of alphabet {0}")]
    NotPermutation(AlphabetId),
    /// The shared discrete permutation-rank adapter rejected the request.
    #[error(transparent)]
    Rank(#[from] RankAdapterError),
}

/// Failure while validating or composing a finite ordinal map.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum OrdinalMapError {
    /// An ordinal selected a position outside the finite domain.
    #[error("ordinal map output {output} selects input {input} outside 0..{cardinality}")]
    OutOfRange {
        /// Output position containing the invalid ordinal.
        output: usize,
        /// Rejected input position.
        input: usize,
        /// Finite domain cardinality.
        cardinality: usize,
    },
    /// Two output positions selected the same input position.
    #[error("ordinal map selects input {input} at outputs {first_output} and {duplicate_output}")]
    DuplicateInput {
        /// Repeated input position.
        input: usize,
        /// First output selecting it.
        first_output: usize,
        /// Later output selecting it.
        duplicate_output: usize,
    },
    /// A map was applied to a slice with another cardinality.
    #[error("ordinal map expects cardinality {expected}, got {found}")]
    CardinalityMismatch {
        /// Validated map cardinality.
        expected: usize,
        /// Supplied slice cardinality.
        found: usize,
    },
    /// Two maps over different domains cannot be composed.
    #[error("cannot compose ordinal maps of cardinalities {first} and {second}")]
    CompositionCardinalityMismatch {
        /// First map cardinality.
        first: usize,
        /// Second map cardinality.
        second: usize,
    },
}

/// Failure while validating an ordered block partition.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum BlockPartitionError {
    /// One declared block contained no positions.
    #[error("block partition block {block} must not be empty")]
    EmptyBlock {
        /// Position of the empty block in declaration order.
        block: usize,
    },
    /// Flattened blocks did not cover the declared finite domain.
    #[error("block partition expects {expected} positions, got {found}")]
    CardinalityMismatch {
        /// Declared source cardinality.
        expected: usize,
        /// Number of declared positions.
        found: usize,
    },
    /// Contiguous block lengths overflowed the platform cardinality.
    #[error("block partition cardinality exceeds the supported series length")]
    CardinalityOverflow,
    /// The flattened block order was not a complete ordinal bijection.
    #[error(transparent)]
    OrdinalMap(#[from] OrdinalMapError),
}

/// Failure while validating a caller-supplied alphabet bijection.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SymbolBijectionError {
    /// A source or target alphabet was invalid.
    #[error(transparent)]
    Alphabet(#[from] AlphabetError),
    /// Source and target alphabets had different cardinalities.
    #[error(
        "symbol bijection source cardinality {source_cardinality} differs from target {target_cardinality}"
    )]
    CardinalityMismatch {
        /// Source alphabet cardinality.
        source_cardinality: usize,
        /// Target alphabet or map cardinality.
        target_cardinality: usize,
    },
    /// A supplied source symbol was outside the source alphabet.
    #[error("symbol bijection contains a foreign source symbol for alphabet {alphabet_id}")]
    ForeignSourceSymbol {
        /// Source alphabet identity.
        alphabet_id: AlphabetId,
    },
    /// A supplied target symbol was outside the target alphabet.
    #[error("symbol bijection contains a foreign target symbol for alphabet {alphabet_id}")]
    ForeignTargetSymbol {
        /// Target alphabet identity.
        alphabet_id: AlphabetId,
    },
    /// One source position was declared more than once.
    #[error("symbol bijection repeats source position {position}")]
    DuplicateSource {
        /// Repeated source position.
        position: usize,
    },
    /// Two source symbols selected the same target position.
    #[error("symbol bijection repeats target position {position}")]
    DuplicateTarget {
        /// Repeated target position.
        position: usize,
    },
    /// A source position had no mapping.
    #[error("symbol bijection is missing source position {position}")]
    MissingSource {
        /// Unmapped source position.
        position: usize,
    },
    /// A target position had no preimage.
    #[error("symbol bijection is missing target position {position}")]
    MissingTarget {
        /// Unmapped target position.
        position: usize,
    },
    /// The internal ordinal spelling was not a bijection.
    #[error(transparent)]
    OrdinalMap(#[from] OrdinalMapError),
}

/// Failure while applying, inverting, or composing a series transform.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SeriesTransformError {
    /// A positional permutation was invalid for the series.
    #[error(transparent)]
    OrdinalMap(#[from] OrdinalMapError),
    /// A symbolic relabeling was not a complete bijection.
    #[error(transparent)]
    Bijection(#[from] SymbolBijectionError),
    /// Aggregate data could not be rebound to the target alphabet.
    #[error(transparent)]
    Rule(#[from] AggregateRuleError),
    /// The transformed value did not satisfy its preserved aggregate rule.
    #[error(transparent)]
    Series(#[from] SeriesError),
    /// An alphabet-bound relabeling was applied to another source alphabet.
    #[error("transform expects source alphabet {expected}, got {found}")]
    SourceAlphabetMismatch {
        /// Alphabet bound into the relabeling.
        expected: AlphabetId,
        /// Alphabet retained by the supplied series.
        found: AlphabetId,
    },
    /// Two alphabet relabelings did not meet at the same intermediate alphabet.
    #[error("cannot compose relabeling to {first_target} with relabeling from {second_source}")]
    CompositionAlphabetMismatch {
        /// Target of the first relabeling.
        first_target: AlphabetId,
        /// Source of the second relabeling.
        second_source: AlphabetId,
    },
    /// Private rule data disagreed with its public rule category.
    #[error("aggregate rule data is missing for category {0:?}")]
    RuleKindMismatch(AggregateRuleKind),
}
