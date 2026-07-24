use sim_lib_pitch_core::PitchClass;

use crate::PitchClassMask;

/// Conventional pitch-set equivalence policy.
///
/// This is intentionally distinct from [`PitchClassMask::normalize`], whose
/// numeric mask identity remains source-compatible.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SetEquivalence {
    /// Compare set classes under transposition only.
    Transposition,
    /// Compare set classes under transposition and inversion.
    TranspositionInversion,
}

/// Conventional pitch-set class identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SetClass {
    /// Forte-style normal order, transposed so the first pitch class is C.
    pub normal: Vec<PitchClass>,
    /// Prime form under the selected [`SetEquivalence`] policy.
    pub prime: Vec<PitchClass>,
    /// The equivalence policy used to produce `prime`.
    pub equivalence: SetEquivalence,
}

/// Classifies a pitch-class mask using conventional normal-order and prime-form
/// identity.
pub fn classify_set(set: PitchClassMask, equivalence: SetEquivalence) -> SetClass {
    let normal = normal_order(set.pitch_classes());
    let prime = match equivalence {
        SetEquivalence::Transposition => normal.clone(),
        SetEquivalence::TranspositionInversion => {
            let inverted = normal_order(set.invert(PitchClass::C).pitch_classes());
            if compare_prime_forms(&inverted, &normal).is_lt() {
                inverted
            } else {
                normal.clone()
            }
        }
    };
    SetClass {
        normal,
        prime,
        equivalence,
    }
}

/// A derived Forte-compatible fact for a pitch-set class.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ForteFact {
    /// Forte set-class label when this crate carries a named example for it.
    pub label: &'static str,
    /// Cardinality of the set class.
    pub cardinality: u8,
    /// Ordinal part of the Forte label.
    pub ordinal: u8,
    /// Prime form associated with this fact.
    pub prime: Vec<PitchClass>,
}

/// Returns a small Forte-compatible example fact when the class is one this
/// crate names explicitly.
pub fn forte_fact_for(set: PitchClassMask) -> Option<ForteFact> {
    let prime = classify_set(set, SetEquivalence::TranspositionInversion).prime;
    let (label, cardinality, ordinal) = match pitch_class_values(&prime).as_slice() {
        [0, 3, 7] => ("3-11A", 3, 11),
        [0, 4, 7] => ("3-11B", 3, 11),
        [0, 1, 4, 6] => ("4-Z15", 4, 15),
        [0, 1, 3, 7] => ("4-Z29", 4, 29),
        [0, 1, 4, 6, 8, 9] => ("6-Z17", 6, 17),
        [0, 1, 3, 4, 6, 8] => ("6-Z43", 6, 43),
        _ => return None,
    };
    Some(ForteFact {
        label,
        cardinality,
        ordinal,
        prime,
    })
}

pub(crate) fn normal_order(mut pitch_classes: Vec<PitchClass>) -> Vec<PitchClass> {
    if pitch_classes.len() <= 1 {
        return pitch_classes;
    }
    pitch_classes.sort_by_key(|pitch_class| pitch_class.value());
    pitch_classes.dedup();

    let mut best: Option<Vec<PitchClass>> = None;
    for start in 0..pitch_classes.len() {
        let candidate = rotate_pitch_classes(&pitch_classes, start);
        if best
            .as_ref()
            .is_none_or(|current| compare_normal_orders(&candidate, current).is_lt())
        {
            best = Some(candidate);
        }
    }
    transpose_to_zero(&best.expect("non-empty pitch-class list has a rotation"))
}

fn rotate_pitch_classes(pitch_classes: &[PitchClass], start: usize) -> Vec<PitchClass> {
    let root = pitch_classes[start].value();
    (0..pitch_classes.len())
        .map(|offset| {
            let index = (start + offset) % pitch_classes.len();
            let value = pitch_classes[index].value();
            let wrapped = if index < start { value + 12 } else { value };
            PitchClass::new(wrapped - root).expect("normal-order rotation folds to pitch class")
        })
        .collect()
}

fn transpose_to_zero(pitch_classes: &[PitchClass]) -> Vec<PitchClass> {
    let Some(first) = pitch_classes.first() else {
        return Vec::new();
    };
    let transposition = -i32::from(first.value());
    pitch_classes
        .iter()
        .map(|pitch_class| pitch_class.transpose(transposition))
        .collect()
}

fn compare_normal_orders(left: &[PitchClass], right: &[PitchClass]) -> std::cmp::Ordering {
    compare_by_span_then_packed(left, right, true)
}

fn compare_prime_forms(left: &[PitchClass], right: &[PitchClass]) -> std::cmp::Ordering {
    compare_by_span_then_packed(left, right, false)
}

fn compare_by_span_then_packed(
    left: &[PitchClass],
    right: &[PitchClass],
    prefer_right_packed: bool,
) -> std::cmp::Ordering {
    for index in (1..left.len()).rev() {
        let ordering = left[index].value().cmp(&right[index].value());
        if !ordering.is_eq() {
            return ordering;
        }
    }
    if prefer_right_packed {
        for index in 1..left.len() {
            let ordering = right[index].value().cmp(&left[index].value());
            if !ordering.is_eq() {
                return ordering;
            }
        }
    }
    pitch_class_values(left).cmp(&pitch_class_values(right))
}

fn pitch_class_values(pitch_classes: &[PitchClass]) -> Vec<u8> {
    pitch_classes
        .iter()
        .map(|pitch_class| pitch_class.value())
        .collect()
}
