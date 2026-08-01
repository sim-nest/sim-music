//! Evidence returned by every accepted series transform.

use crate::{AlphabetId, OrdinalMap, SerialAlphabet, Series, SeriesTransform};

/// A source invariant explicitly relaxed by a transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelaxedInvariant {
    /// Source positions were reordered.
    SourceOrder,
    /// Source symbols were replaced through a validated bijection.
    SymbolIdentity,
    /// The result belongs to a differently identified alphabet.
    AlphabetIdentity,
}

/// Algebra evidence for one successfully applied transform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformCertificate<A: SerialAlphabet> {
    /// Stable identity of the validated source alphabet.
    pub source_alphabet: AlphabetId,
    /// Stable identity of the validated target alphabet.
    pub target_alphabet: AlphabetId,
    /// Whether aggregate policy and counts were preserved under the bijections.
    pub aggregate_preserved: bool,
    /// Exact output-position to source-position map used by the transform.
    pub order_map: OrdinalMap,
    /// Exact inverse operation when the transform algebra supplies one.
    pub inverse: Option<SeriesTransform<A>>,
    /// Source invariants intentionally relaxed by this operation.
    pub relaxed_invariants: Vec<RelaxedInvariant>,
}

/// A transformed series paired with the evidence that justifies it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformedSeries<A: SerialAlphabet> {
    /// Valid target series.
    pub series: Series<A>,
    /// Source/target, order, inverse, and preservation evidence.
    pub certificate: TransformCertificate<A>,
}
