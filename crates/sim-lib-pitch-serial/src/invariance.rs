//! Independent ordered and unordered invariance comparisons for row segments.

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::analyze_set_relations;

use crate::RowSegment;

/// Independent invariance facts relating two ordered row segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentInvariant {
    /// Whether the compared segments use the same source ordinals in the same order.
    pub ordinal_identity: bool,
    /// Whether the compared segments contain the same pitch classes in the same order.
    pub pitch_identity: bool,
    /// Exact `Tn` preserving the segment's ordered pitch classes, when present.
    pub transposition: Option<u8>,
    /// Exact `TnI` preserving the segment's ordered pitch classes, when present.
    pub inversion: Option<u8>,
    /// Whether the directed ordered-interval strings match exactly.
    pub interval_order_identity: bool,
    /// Whether the unordered pitch-class projections share one set class.
    pub set_class_identity: bool,
}

/// Compares two ordered row segments without conflating ordered and unordered facts.
pub fn analyze_invariance(left: &RowSegment, right: &RowSegment) -> SegmentInvariant {
    let relation = analyze_set_relations(left.mask(), right.mask());
    SegmentInvariant {
        ordinal_identity: left.ordinals() == right.ordinals(),
        pitch_identity: left.classes() == right.classes(),
        transposition: ordered_transposition(left.classes(), right.classes()),
        inversion: ordered_inversion(left.classes(), right.classes()),
        interval_order_identity: left.ordered_intervals() == right.ordered_intervals(),
        set_class_identity: relation.transposition_inversion_equivalent,
    }
}

fn ordered_transposition(left: &[PitchClass], right: &[PitchClass]) -> Option<u8> {
    if left.len() != right.len() {
        return None;
    }
    let Some((first_left, first_right)) = left.first().zip(right.first()) else {
        return Some(0);
    };
    let shift = (12 + i16::from(first_right.value()) - i16::from(first_left.value())) as u8 % 12;
    left.iter()
        .zip(right)
        .all(|(source, target)| source.transpose(i32::from(shift)) == *target)
        .then_some(shift)
}

fn ordered_inversion(left: &[PitchClass], right: &[PitchClass]) -> Option<u8> {
    if left.len() != right.len() {
        return None;
    }
    let Some((first_left, first_right)) = left.first().zip(right.first()) else {
        return Some(0);
    };
    let index = (first_left.value() + first_right.value()) % 12;
    left.iter()
        .zip(right)
        .all(|(source, target)| target.value() == (12 + index - source.value()) % 12)
        .then_some(index)
}
