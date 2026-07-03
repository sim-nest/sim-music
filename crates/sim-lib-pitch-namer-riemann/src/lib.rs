//! Neo-Riemannian / functional triad naming for the SIM music libraries.
//!
//! This crate implements the Riemannian naming school: it labels a triad by its
//! functional quality relative to its root, distinguishing a major triad (`T`,
//! upper-case for the major tonic function) from a minor triad (`t`,
//! lower-case). [`label_riemann`] rotates the pitch-class set so the root sits at
//! zero and matches the major or minor triad signature; non-triadic sets return
//! `None`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_set::PitchClassMask;

/// Labels `mask` as a major (`T`) or minor (`t`) triad relative to its root.
///
/// When `root` is `None` the lowest pitch class of the set is used. Returns `None`
/// if the set is not a major or minor triad.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::PitchClass;
/// use sim_lib_pitch_set::PitchClassMask;
/// use sim_lib_pitch_namer_riemann::label_riemann;
///
/// let triad = PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]);
/// assert_eq!(label_riemann(triad, Some(PitchClass::C)).as_deref(), Some("T(C)"));
/// ```
pub fn label_riemann(mask: PitchClassMask, root: Option<PitchClass>) -> Option<String> {
    let root = root.or_else(|| mask.pitch_classes().into_iter().next())?;
    let normalized = mask.rotate(-(root.0 as i32));
    let quality = match normalized.0 & 0x0fff {
        0b0000_1001_0001 => "T",
        0b0000_1000_1001 => "t",
        _ => return None,
    };
    Some(format!("{quality}({})", root.canonical_name()))
}

#[cfg(test)]
mod tests {
    use sim_lib_pitch_core::PitchClass;
    use sim_lib_pitch_set::PitchClassMask;

    use crate::label_riemann;

    #[test]
    fn labels_major_triad() {
        let label = label_riemann(
            PitchClassMask::from_pitch_classes(&[PitchClass::C, PitchClass::E, PitchClass::G]),
            Some(PitchClass::C),
        )
        .expect("riemann label");
        assert_eq!(label, "T(C)");
    }
}
