//! Roman-numeral harmonic analysis for the SIM music libraries.
//!
//! This crate implements the functional roman-numeral naming school: given a key,
//! it labels a chord by its scale degree (`I` through `VII`) and quality, using
//! upper-case numerals for major chords, lower-case for minor, an `o` suffix for
//! diminished, and `7`/`maj7` suffixes for sevenths (for example `V7` for a
//! dominant seventh in a major key). [`label_roman`] requires a key context and
//! returns a diagnostic string on failure.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_scale::{Key, Scale};
use sim_lib_pitch_set::PitchClassMask;

/// Labels `mask` as a roman numeral relative to `key`.
///
/// The chord root is taken from `preferred_root`, or the lowest pitch class of the
/// set if absent. Returns `Err` with a diagnostic when no key is supplied, the
/// root cannot be determined, or the root lies outside the key.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::PitchClass;
/// use sim_lib_pitch_scale::{Key, Mode};
/// use sim_lib_pitch_set::PitchClassMask;
/// use sim_lib_pitch_namer_roman::label_roman;
///
/// let g7 = PitchClassMask::from_pitch_classes(&[
///     PitchClass::G,
///     PitchClass::B,
///     PitchClass::D,
///     PitchClass::F,
/// ]);
/// let key = Key { tonic: PitchClass::C, mode: Mode::Major };
/// assert_eq!(label_roman(g7, Some(key), Some(PitchClass::G)).unwrap(), "V7");
/// ```
pub fn label_roman(
    mask: PitchClassMask,
    key: Option<Key>,
    preferred_root: Option<PitchClass>,
) -> Result<String, String> {
    let key = key.ok_or_else(|| "key context required".to_owned())?;
    let scale = Scale::new(key.tonic, key.mode);
    let root = preferred_root
        .or_else(|| choose_root(mask))
        .ok_or_else(|| "cannot determine root".to_owned())?;
    let degree = scale
        .degree_of(root)
        .ok_or_else(|| "root outside key".to_owned())?;
    let numeral = match degree {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "?",
    };
    let quality = chord_quality(mask, root);
    let text = match quality {
        Some("maj") => numeral.to_owned(),
        Some("min") => numeral.to_ascii_lowercase(),
        Some("dim") => format!("{}o", numeral.to_ascii_lowercase()),
        Some("dom7") => format!("{numeral}7"),
        Some("maj7") => format!("{numeral}maj7"),
        Some("min7") => format!("{}7", numeral.to_ascii_lowercase()),
        _ => format!("{numeral}?"),
    };
    Ok(text)
}

fn choose_root(mask: PitchClassMask) -> Option<PitchClass> {
    mask.pitch_classes().into_iter().next()
}

fn chord_quality(mask: PitchClassMask, root: PitchClass) -> Option<&'static str> {
    let normalized = mask.rotate(-i32::from(root.value()));
    match normalized.bits() {
        0b0000_1001_0001 => Some("maj"),
        0b0000_1000_1001 => Some("min"),
        0b0000_0100_1001 => Some("dim"),
        0b1000_1001_0001 => Some("maj7"),
        0b0100_1001_0001 => Some("dom7"),
        0b0100_1000_1001 => Some("min7"),
        _ => None,
    }
}

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests {
    use sim_lib_pitch_core::PitchClass;
    use sim_lib_pitch_scale::{Key, Mode};
    use sim_lib_pitch_set::PitchClassMask;

    use crate::label_roman;

    #[test]
    fn labels_major_dominant_in_key() {
        let label = label_roman(
            PitchClassMask::from_pitch_classes(&[
                PitchClass::G,
                PitchClass::B,
                PitchClass::D,
                PitchClass::F,
            ]),
            Some(Key {
                tonic: PitchClass::C,
                mode: Mode::Major,
            }),
            Some(PitchClass::G),
        )
        .expect("roman label");
        assert_eq!(label, "V7");
    }
}
