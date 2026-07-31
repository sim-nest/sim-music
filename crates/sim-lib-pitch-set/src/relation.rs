//! Typed relations between canonical pitch-class set forms.

use crate::{IntervalVector, PitchClassMask, SetClass, SetEquivalence, classify_set};

/// Exact inclusion relation between two pitch-class masks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SetInclusion {
    /// The masks contain exactly the same pitch classes.
    Equal,
    /// Every source pitch class occurs in the target, and the target has more.
    ProperSubset,
    /// Every target pitch class occurs in the source, and the source has more.
    ProperSuperset,
    /// The masks share pitch classes, but neither contains the other.
    Overlap,
    /// The masks have no pitch classes in common.
    Disjoint,
}

/// Reproducible set-theory evidence relating two pitch-class masks.
///
/// Operator lists describe exact mask-to-mask transformations. Equivalence
/// booleans are instead derived from the conventional phase-08 canonical forms,
/// so callers never need to compare display labels or numeric normalization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetRelationAnalysis {
    /// Source set class under transposition-and-inversion equivalence.
    pub source_class: SetClass,
    /// Target set class under transposition-and-inversion equivalence.
    pub target_class: SetClass,
    /// Exact inclusion relation between the source and target masks.
    pub inclusion: SetInclusion,
    /// Pitch classes present in both masks.
    pub common_tones: PitchClassMask,
    /// Transposition amounts `Tn` that map the source mask exactly to the target.
    pub transpositions: Vec<u8>,
    /// Conventional inversion indices `TnI` mapping source exactly to target.
    pub inversion_indices: Vec<u8>,
    /// Whether conventional transposition-only prime forms agree.
    pub transposition_equivalent: bool,
    /// Whether conventional transposition-and-inversion prime forms agree.
    pub transposition_inversion_equivalent: bool,
    /// Whether the target is the source's exact aggregate complement.
    pub exact_complement: bool,
    /// Whether the target is Tn/TnI-equivalent to the source's complement.
    pub complement_equivalent: bool,
    /// Whether the masks have equal interval vectors but distinct Tn/TnI forms.
    pub z_related: bool,
    /// Source interval-class census.
    pub source_interval_vector: IntervalVector,
    /// Target interval-class census.
    pub target_interval_vector: IntervalVector,
}

/// Analyzes exact and canonical relations between two pitch-class masks.
///
/// This reports every exact `Tn` and `TnI` operator rather than choosing one,
/// which preserves symmetric-set evidence. Conventional equivalence and
/// complement equivalence are computed from [`SetEquivalence`] prime forms.
pub fn analyze_set_relations(
    source: PitchClassMask,
    target: PitchClassMask,
) -> SetRelationAnalysis {
    let common_tones = source.intersection(target);
    let inclusion = if source == target {
        SetInclusion::Equal
    } else if source.is_subset_of(target) {
        SetInclusion::ProperSubset
    } else if source.is_superset_of(target) {
        SetInclusion::ProperSuperset
    } else if source.is_disjoint_from(target) {
        SetInclusion::Disjoint
    } else {
        SetInclusion::Overlap
    };

    let transpositions = (0..12)
        .filter(|shift| source.rotate(i32::from(*shift)) == target)
        .collect();
    let inversion_indices = (0..12)
        .filter(|index| source.invert_tni(*index) == target)
        .collect();

    let source_t = classify_set(source, SetEquivalence::Transposition);
    let target_t = classify_set(target, SetEquivalence::Transposition);
    let source_class = classify_set(source, SetEquivalence::TranspositionInversion);
    let target_class = classify_set(target, SetEquivalence::TranspositionInversion);
    let complement_class =
        classify_set(source.complement(), SetEquivalence::TranspositionInversion);
    let source_interval_vector = source.interval_vector();
    let target_interval_vector = target.interval_vector();

    SetRelationAnalysis {
        source_class,
        target_class: target_class.clone(),
        inclusion,
        common_tones,
        transpositions,
        inversion_indices,
        transposition_equivalent: source_t.prime == target_t.prime,
        transposition_inversion_equivalent: source_class_prime_eq(source, target),
        exact_complement: source.complement() == target,
        complement_equivalent: complement_class.prime == target_class.prime,
        z_related: source.is_z_related_to(target),
        source_interval_vector,
        target_interval_vector,
    }
}

fn source_class_prime_eq(source: PitchClassMask, target: PitchClassMask) -> bool {
    classify_set(source, SetEquivalence::TranspositionInversion).prime
        == classify_set(target, SetEquivalence::TranspositionInversion).prime
}
