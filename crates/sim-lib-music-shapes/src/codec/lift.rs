use std::str::FromStr;

use sim_lib_music_lift::{
    CounterpointLiftOpts, LabelStrategy, ProgressionLiftOpts, VoiceAssignment,
};
use sim_lib_pitch_scale::Key;

use super::{MusicShapeError, analysis::*, decode_mode};
use crate::codec::parse::decode_time;
use sim_lib_music_core::parse_pitch;

/// Decodes a `#(LabelStrategy ...)` form into a `LabelStrategy`.
pub fn decode_label_strategy(value: &str) -> Result<LabelStrategy, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "LabelStrategy")?;
    match field_atom(&node, "value")?.as_str() {
        "Functional" => Ok(LabelStrategy::Functional),
        "JazzChord" => Ok(LabelStrategy::JazzChord),
        "SetClass" => Ok(LabelStrategy::SetClass),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

/// Decodes a `#(VoiceAssignment ...)` form into a `VoiceAssignment`.
pub fn decode_voice_assignment(value: &str) -> Result<VoiceAssignment, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "VoiceAssignment")?;
    match field_atom(&node, "value")?.as_str() {
        "ChannelOnly" => Ok(VoiceAssignment::ChannelOnly),
        "TrackThenChannel" => Ok(VoiceAssignment::TrackThenChannel),
        "HighestFirst" => Ok(VoiceAssignment::HighestFirst),
        "LowestFirst" => Ok(VoiceAssignment::LowestFirst),
        _ => Err(MusicShapeError::InvalidMusic),
    }
}

/// Decodes a `#(ProgressionLiftOpts ...)` form into a `ProgressionLiftOpts`.
pub fn decode_progression_lift_opts(value: &str) -> Result<ProgressionLiftOpts, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "ProgressionLiftOpts")?;
    Ok(ProgressionLiftOpts {
        grid: decode_time(&field_atom(&node, "grid")?)?,
        min_notes: usize::from_str(&field_atom(&node, "min_notes")?)
            .map_err(|_| MusicShapeError::InvalidMusic)?,
        key_hint: decode_key_hint(&field_atom(&node, "key_hint")?)?,
        label_strategy: decode_label_strategy(&format!(
            "#(LabelStrategy value={})",
            field_atom(&node, "label_strategy")?
        ))?,
        window_mode: decode_chord_window_mode(&format!(
            "#(ChordWindowMode value={})",
            field_atom(&node, "window_mode")?
        ))?,
    })
}

/// Decodes a `#(CounterpointLiftOpts ...)` form into a `CounterpointLiftOpts`.
pub fn decode_counterpoint_lift_opts(value: &str) -> Result<CounterpointLiftOpts, MusicShapeError> {
    let node = parse_node(value)?;
    ensure_form(&node, "CounterpointLiftOpts")?;
    Ok(CounterpointLiftOpts {
        min_rest_to_close: decode_time(&field_atom(&node, "min_rest_to_close")?)?,
        max_voices_per_track: usize::from_str(&field_atom(&node, "max_voices_per_track")?)
            .map_err(|_| MusicShapeError::InvalidMusic)?,
        voice_assignment: decode_voice_assignment(&format!(
            "#(VoiceAssignment value={})",
            field_atom(&node, "voice_assignment")?
        ))?,
    })
}

fn decode_key_hint(value: &str) -> Result<Option<Key>, MusicShapeError> {
    if value == "none" {
        return Ok(None);
    }
    let (tonic, mode) = value.split_once(':').ok_or(MusicShapeError::InvalidMusic)?;
    let tonic = parse_pitch(&format!("{tonic}4")).map_err(|_| MusicShapeError::InvalidMusic)?;
    let mode = decode_mode(mode).ok_or(MusicShapeError::InvalidMusic)?;
    Ok(Some(Key {
        tonic: tonic.class,
        mode,
    }))
}
