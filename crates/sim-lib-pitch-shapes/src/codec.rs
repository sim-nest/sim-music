use std::str::FromStr;

use thiserror::Error;

use sim_lib_pitch_chord::{Chord, ChordSymbol, PitchChordError};
use sim_lib_pitch_core::{Interval, Pitch, PitchError, parse_interval, parse_pitch};
use sim_lib_pitch_scale::{Key, Mode, Scale};
use sim_lib_pitch_set::PitchClassMask;

/// Error returned when encoding or decoding a pitch shape text form fails.
#[derive(Debug, Error)]
pub enum PitchShapeError {
    /// An underlying pitch or interval parse error.
    #[error(transparent)]
    Pitch(#[from] PitchError),
    /// An underlying chord parse error.
    #[error(transparent)]
    Chord(#[from] PitchChordError),
    /// A pitch-class mask form was malformed.
    #[error("invalid pitch-class mask")]
    InvalidPitchClassMask,
    /// A mode name was not recognized.
    #[error("invalid mode")]
    InvalidMode,
    /// A scale form was malformed.
    #[error("invalid scale")]
    InvalidScale,
    /// A chord form was malformed or empty.
    #[error("invalid chord")]
    InvalidChord,
}

/// Encodes a [`Pitch`] as its canonical chroma-plus-octave string (for example
/// `"C4"`).
pub fn encode_pitch(pitch: Pitch) -> String {
    format!("{}{}", pitch.class.canonical_name(), pitch.octave)
}

/// Decodes a pitch string such as `"C4"` or `"Eb5"` into a [`Pitch`].
pub fn decode_pitch(value: &str) -> Result<Pitch, PitchShapeError> {
    Ok(parse_pitch(value)?)
}

/// Encodes an [`Interval`] as a named token (`"m3"`, `"TT"`, `"P5"`, `"M7"`) or a
/// `#(Interval n)` form for other sizes.
pub fn encode_interval(interval: Interval) -> String {
    match interval.semitones {
        3 => "m3".to_owned(),
        6 => "TT".to_owned(),
        7 => "P5".to_owned(),
        11 => "M7".to_owned(),
        other => format!("#(Interval {other})"),
    }
}

/// Decodes a named interval token or `#(Interval n)` form into an [`Interval`].
pub fn decode_interval(value: &str) -> Result<Interval, PitchShapeError> {
    if let Ok(interval) = parse_interval(value) {
        return Ok(interval);
    }
    let inner = value
        .strip_prefix("#(Interval ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(PitchShapeError::Pitch(PitchError::InvalidInterval))?;
    Ok(Interval {
        semitones: i32::from_str(inner).map_err(|_| PitchError::InvalidInterval)?,
    })
}

/// Encodes a [`PitchClassMask`] as a `#(PitchClassMask bits)` form using its low
/// twelve bits.
pub fn encode_pitch_class_mask(mask: PitchClassMask) -> String {
    format!("#(PitchClassMask {})", mask.0 & 0x0fff)
}

/// Decodes a `#(PitchClassMask bits)` form into a [`PitchClassMask`].
pub fn decode_pitch_class_mask(value: &str) -> Result<PitchClassMask, PitchShapeError> {
    let inner = value
        .strip_prefix("#(PitchClassMask ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(PitchShapeError::InvalidPitchClassMask)?;
    let bits = u16::from_str(inner).map_err(|_| PitchShapeError::InvalidPitchClassMask)?;
    Ok(PitchClassMask(bits & 0x0fff))
}

/// Encodes a [`Mode`] as its canonical name (for example `"dorian"`).
pub fn encode_mode(mode: Mode) -> &'static str {
    mode.name()
}

/// Decodes a mode name into a [`Mode`].
pub fn decode_mode(value: &str) -> Result<Mode, PitchShapeError> {
    match value {
        "major" => Ok(Mode::Major),
        "minor-natural" => Ok(Mode::MinorNatural),
        "minor-harmonic" => Ok(Mode::MinorHarmonic),
        "minor-melodic" => Ok(Mode::MinorMelodic),
        "dorian" => Ok(Mode::Dorian),
        "phrygian" => Ok(Mode::Phrygian),
        "lydian" => Ok(Mode::Lydian),
        "mixolydian" => Ok(Mode::Mixolydian),
        "aeolian" => Ok(Mode::Aeolian),
        "locrian" => Ok(Mode::Locrian),
        "whole-tone" => Ok(Mode::WholeTone),
        "diminished" => Ok(Mode::Diminished),
        "chromatic" => Ok(Mode::Chromatic),
        _ => Err(PitchShapeError::InvalidMode),
    }
}

/// Encodes a [`Scale`] as a `tonic:mode` string (for example `"C:major"`).
pub fn encode_scale(scale: Scale) -> String {
    format!(
        "{}:{}",
        scale.tonic.canonical_name(),
        encode_mode(scale.mode)
    )
}

/// Decodes a `tonic:mode` string into a [`Scale`].
pub fn decode_scale(value: &str) -> Result<Scale, PitchShapeError> {
    let (tonic, mode) = value.split_once(':').ok_or(PitchShapeError::InvalidScale)?;
    Ok(Scale::new(
        parse_pitch(&format!("{tonic}4"))?.class,
        decode_mode(mode)?,
    ))
}

/// Encodes a [`Key`] as a `tonic:mode` string, sharing the scale encoding.
pub fn encode_key(key: Key) -> String {
    encode_scale(Scale::new(key.tonic, key.mode))
}

/// Decodes a `tonic:mode` string into a [`Key`].
pub fn decode_key(value: &str) -> Result<Key, PitchShapeError> {
    let scale = decode_scale(value)?;
    Ok(Key {
        tonic: scale.tonic,
        mode: scale.mode,
    })
}

/// Encodes a [`ChordSymbol`] as its text label (for example `"Am7/C"`).
pub fn encode_chord_symbol(symbol: &ChordSymbol) -> String {
    let quality = if symbol.quality == "maj" {
        ""
    } else {
        symbol.quality
    };
    match symbol.slash_bass {
        Some(bass) => {
            format!(
                "{}{}/{}",
                symbol.root.canonical_name(),
                quality,
                bass.canonical_name()
            )
        }
        None => format!("{}{}", symbol.root.canonical_name(), quality),
    }
}

/// Decodes a chord-symbol string into a [`ChordSymbol`].
pub fn decode_chord_symbol(value: &str) -> Result<ChordSymbol, PitchShapeError> {
    Ok(ChordSymbol::parse(value)?)
}

/// Encodes a [`Chord`] as a comma-separated list of pitch strings.
pub fn encode_chord(chord: &Chord) -> String {
    chord
        .pitches()
        .into_iter()
        .map(encode_pitch)
        .collect::<Vec<_>>()
        .join(",")
}

/// Decodes a comma-separated list of pitch strings into a [`Chord`].
pub fn decode_chord(value: &str) -> Result<Chord, PitchShapeError> {
    let notes = value
        .split(',')
        .filter(|part| !part.is_empty())
        .map(decode_pitch)
        .collect::<Result<Vec<_>, _>>()?;
    if notes.is_empty() {
        return Err(PitchShapeError::InvalidChord);
    }
    Ok(Chord::new(notes))
}
