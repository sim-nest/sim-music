use thiserror::Error;

use sim_lib_pitch_core::{PitchClass, parse_pitch};
use sim_lib_pitch_set::PitchClassMask;

use crate::{JazzChordSymbol, JazzQuality};

/// Error returned when a jazz chord symbol cannot be parsed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JazzError {
    /// The input was not a recognizable jazz chord symbol.
    #[error("invalid jazz chord symbol")]
    InvalidSymbol,
}

/// Parses a jazz chord symbol such as `"Cmaj7"`, `"Am7"`, or `"G7/B"` into a
/// [`JazzChordSymbol`].
///
/// A colon may separate root and quality (`"C:maj7"`), and a `/` introduces a
/// slash bass. Returns [`JazzError::InvalidSymbol`] on unrecognized input.
///
/// # Examples
///
/// ```
/// use sim_lib_pitch_core::PitchClass;
/// use sim_lib_pitch_namer_jazz::{parse_jazz_symbol, JazzQuality};
///
/// let symbol = parse_jazz_symbol("Cmaj7").unwrap();
/// assert_eq!(symbol.root, PitchClass::C);
/// assert_eq!(symbol.quality, JazzQuality::Major7);
/// ```
pub fn parse_jazz_symbol(value: &str) -> Result<JazzChordSymbol, JazzError> {
    let (head, slash_bass) = match value.split_once('/') {
        Some((head, bass)) => (head, Some(parse_pitch_class(bass)?)),
        None => (value, None),
    };
    let (root_text, quality_text) = match head.split_once(':') {
        Some((root, quality)) => (root, quality),
        None => {
            let split = if head
                .as_bytes()
                .get(1)
                .is_some_and(|byte| matches!(*byte, b'#' | b'b' | b's'))
            {
                2
            } else {
                1
            };
            (&head[..split], &head[split..])
        }
    };
    Ok(JazzChordSymbol {
        root: parse_pitch_class(root_text)?,
        quality: parse_quality(quality_text)?,
        slash_bass,
    })
}

fn parse_pitch_class(value: &str) -> Result<PitchClass, JazzError> {
    Ok(parse_pitch(&format!("{value}4"))
        .map_err(|_| JazzError::InvalidSymbol)?
        .class)
}

fn parse_quality(value: &str) -> Result<JazzQuality, JazzError> {
    match value {
        "" | "maj" => Ok(JazzQuality::Major),
        "m" | "min" => Ok(JazzQuality::Minor),
        "7" => Ok(JazzQuality::Dominant7),
        "maj7" | "M7" => Ok(JazzQuality::Major7),
        "m7" | "min7" => Ok(JazzQuality::Minor7),
        "dim" | "o" => Ok(JazzQuality::Diminished),
        "aug" | "+" => Ok(JazzQuality::Augmented),
        "sus2" => Ok(JazzQuality::Suspended2),
        "sus4" | "sus" => Ok(JazzQuality::Suspended4),
        "6" => Ok(JazzQuality::Sixth),
        "m6" | "min6" => Ok(JazzQuality::Minor6),
        _ => Err(JazzError::InvalidSymbol),
    }
}

/// Recognizes `mask` as a jazz chord by trying each candidate root and quality,
/// returning the first exact match or `None`.
///
/// `preferred_root` is tried before the set's own pitch classes, so a known root
/// biases the recognition.
pub fn match_jazz_symbol(
    mask: PitchClassMask,
    preferred_root: Option<PitchClass>,
) -> Option<JazzChordSymbol> {
    let candidates = preferred_root
        .into_iter()
        .chain(mask.pitch_classes())
        .collect::<Vec<_>>();
    for root in candidates {
        for quality in JazzQuality::all() {
            let symbol = JazzChordSymbol {
                root,
                quality: *quality,
                slash_bass: None,
            };
            if symbol.mask() == mask {
                return Some(symbol);
            }
        }
    }
    None
}
