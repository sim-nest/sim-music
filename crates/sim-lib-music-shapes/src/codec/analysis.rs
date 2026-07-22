use std::str::FromStr;

use sim_codec::{DomainForm, DomainValue, parse_domain_form};
use sim_lib_music_analysis::{ChordWindow, ChordWindowMode, DiffFrame, DiffRoll};
use sim_lib_music_core::parse_pitch;
use sim_lib_music_transform::{FunctionMap, RetrogradeMode};
use sim_lib_pitch_scale::Scale;
use sim_lib_pitch_set::{BitChord, PitchClassMask, PitchRangeMask};

use super::{MusicShapeError, decode_mode};
use crate::codec::parse::decode_time;

/// Decodes a `#(RetrogradeMode ...)` form into a `RetrogradeMode`.
pub fn decode_retrograde_mode(value: &str) -> Result<RetrogradeMode, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "RetrogradeMode")?;
    match field_atom(&node, "value")?.as_str() {
        "Cutout" => Ok(RetrogradeMode::Cutout),
        "PinnedNoteOn" => Ok(RetrogradeMode::PinnedNoteOn),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

/// Decodes a `#(FunctionMap ...)` form into a `FunctionMap`.
pub fn decode_function_map(value: &str) -> Result<FunctionMap, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "FunctionMap")?;
    match field_atom(&node, "kind")?.as_str() {
        "Major" => Ok(FunctionMap::Major),
        "MinorNatural" => Ok(FunctionMap::MinorNatural),
        "MinorHarmonic" => Ok(FunctionMap::MinorHarmonic),
        "MinorMelodicAsc" => Ok(FunctionMap::MinorMelodicAsc),
        "Dorian" => Ok(FunctionMap::Dorian),
        "Phrygian" => Ok(FunctionMap::Phrygian),
        "Lydian" => Ok(FunctionMap::Lydian),
        "Mixolydian" => Ok(FunctionMap::Mixolydian),
        "Locrian" => Ok(FunctionMap::Locrian),
        "Custom" => {
            let tonic = parse_pitch(&format!("{}4", field_atom(&node, "tonic")?))
                .map_err(|_| MusicShapeError::InvalidMusic)?;
            let mode =
                decode_mode(&field_atom(&node, "mode")?).ok_or(MusicShapeError::InvalidMusic)?;
            Ok(FunctionMap::Custom(Scale::new(tonic.class, mode)))
        }
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

/// Decodes a `#(ChordWindowMode ...)` form into a `ChordWindowMode`.
pub fn decode_chord_window_mode(value: &str) -> Result<ChordWindowMode, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "ChordWindowMode")?;
    match field_atom(&node, "value")?.as_str() {
        "SoundingNotes" => Ok(ChordWindowMode::SoundingNotes),
        "StartingNotes" => Ok(ChordWindowMode::StartingNotes),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

/// Decodes a `#(DiffFrame ...)` form into a `DiffFrame`.
pub fn decode_diff_frame(value: &str) -> Result<DiffFrame, MusicShapeError> {
    let node = parse_node(value)?;
    decode_diff_frame_node(&node)
}

/// Decodes a `#(DiffRoll ...)` form into a `DiffRoll`.
pub fn decode_diff_roll(value: &str) -> Result<DiffRoll, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "DiffRoll")?;
    let frames = field_list(&node, "frames")?
        .iter()
        .map(|frame| decode_diff_frame_node(frame.as_form()?))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DiffRoll { frames })
}

/// Decodes a `#(ChordWindow ...)` form into a `ChordWindow`.
pub fn decode_chord_window(value: &str) -> Result<ChordWindow, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "ChordWindow")?;
    let mode = match field_atom(&node, "mode")?.as_str() {
        "SoundingNotes" => ChordWindowMode::SoundingNotes,
        "StartingNotes" => ChordWindowMode::StartingNotes,
        _ => return Err(MusicShapeError::InvalidMusic),
    };
    let range_mask = PitchRangeMask {
        bits: parse_u128(&field_atom(&node, "range")?)?,
    };
    let pitch_class_mask = PitchClassMask::new(parse_u16(&field_atom(&node, "pitch_classes")?)?)
        .map_err(|_| MusicShapeError::InvalidMusic)?;
    let root = match field_atom(&node, "root")?.as_str() {
        "none" => None,
        value => Some(
            parse_pitch(&format!("{value}4"))
                .map_err(|_| MusicShapeError::InvalidMusic)?
                .class,
        ),
    };
    Ok(ChordWindow {
        at: decode_time(&field_atom(&node, "at")?)?,
        until: decode_time(&field_atom(&node, "until")?)?,
        mode,
        range_mask,
        pitch_class_mask,
        bit_chord: BitChord {
            mask: pitch_class_mask,
            root,
        },
    })
}

pub(crate) fn parse_node(value: &str) -> Result<DomainForm, MusicShapeError> {
    Ok(parse_domain_form(value)?)
}

fn decode_diff_frame_node(node: &DomainForm) -> Result<DiffFrame, MusicShapeError> {
    ensure_form(node, "DiffFrame")?;
    Ok(DiffFrame {
        at: decode_time(&field_atom(node, "at")?)?,
        sounding: PitchRangeMask {
            bits: parse_u128(&field_atom(node, "sounding")?)?,
        },
        started: PitchRangeMask {
            bits: parse_u128(&field_atom(node, "started")?)?,
        },
        ended: PitchRangeMask {
            bits: parse_u128(&field_atom(node, "ended")?)?,
        },
        slurred: PitchRangeMask {
            bits: parse_u128(&field_atom(node, "slurred")?)?,
        },
    })
}

pub(crate) fn ensure_form(node: &DomainForm, expected: &str) -> Result<(), MusicShapeError> {
    if node.name == expected {
        Ok(())
    } else {
        Err(MusicShapeError::InvalidMusic)
    }
}

pub(crate) fn field<'a>(
    node: &'a DomainForm,
    name: &str,
) -> Result<&'a DomainValue, MusicShapeError> {
    node.field(name).ok_or(MusicShapeError::InvalidMusic)
}

pub(crate) fn field_atom(node: &DomainForm, name: &str) -> Result<String, MusicShapeError> {
    match field(node, name)? {
        DomainValue::Atom(value) => Ok(value.clone()),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

pub(crate) fn field_list<'a>(
    node: &'a DomainForm,
    name: &str,
) -> Result<&'a [DomainValue], MusicShapeError> {
    match field(node, name)? {
        DomainValue::List(values) => Ok(values),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

fn parse_u16(value: &str) -> Result<u16, MusicShapeError> {
    u16::from_str(value).map_err(|_| MusicShapeError::InvalidMusic)
}

fn parse_u128(value: &str) -> Result<u128, MusicShapeError> {
    u128::from_str(value).map_err(|_| MusicShapeError::InvalidMusic)
}
