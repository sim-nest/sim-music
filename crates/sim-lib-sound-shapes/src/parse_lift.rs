use sim_lib_pitch_core::Pitch;
use sim_lib_sound_audio_lift::{
    AudioLiftFrame, AudioLiftOptions, AudioNoteCandidate, PitchCandidate,
};

use crate::parse::{Node, field_atom, field_form_text, field_list, parse_f64, parse_node};
use crate::{SoundShapeError, decode_amplitude, decode_frequency, decode_spectrum};

/// Decodes [`AudioLiftOptions`] from its sound-shape text form.
pub fn decode_audio_lift_options(value: &str) -> Result<AudioLiftOptions, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(AudioLiftOptions {
        window_size: field_atom(&node, "window_size")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        hop_size: field_atom(&node, "hop_size")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        max_peaks: field_atom(&node, "max_peaks")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        min_peak_ratio: parse_f64(&field_atom(&node, "min_peak_ratio")?)?,
        harmonic_tolerance_cents: parse_f64(&field_atom(&node, "harmonic_tolerance_cents")?)?,
        min_note_confidence: parse_f64(&field_atom(&node, "min_note_confidence")?)?,
        min_note_windows: field_atom(&node, "min_note_windows")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
    })
}

/// Decodes a [`PitchCandidate`] from its sound-shape text form.
pub fn decode_pitch_candidate(value: &str) -> Result<PitchCandidate, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(PitchCandidate {
        pitch: Pitch::from_semitone(
            field_atom(&node, "semitone")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        ),
        frequency: decode_frequency(&field_form_text(&node, "frequency")?)?,
        amplitude: decode_amplitude(&field_form_text(&node, "amplitude")?)?,
        confidence: parse_f64(&field_atom(&node, "confidence")?)?,
        cents_error: parse_f64(&field_atom(&node, "cents_error")?)?,
        harmonic_count: field_atom(&node, "harmonic_count")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
    })
}

/// Decodes an [`AudioLiftFrame`] from its sound-shape text form.
pub fn decode_audio_lift_frame(value: &str) -> Result<AudioLiftFrame, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(AudioLiftFrame {
        index: field_atom(&node, "index")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        onset_sample: field_atom(&node, "onset_sample")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        duration_samples: field_atom(&node, "duration_samples")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        spectrum: decode_spectrum(&field_form_text(&node, "spectrum")?)?,
        pitch_candidates: field_list(&node, "pitch_candidates")?
            .iter()
            .map(Node::render_text)
            .map(|text| decode_pitch_candidate(&text))
            .collect::<Result<Vec<_>, _>>()?,
        diagnostics: string_list(field_list(&node, "diagnostics")?)?,
    })
}

/// Decodes an [`AudioNoteCandidate`] from its sound-shape text form.
pub fn decode_audio_note_candidate(value: &str) -> Result<AudioNoteCandidate, SoundShapeError> {
    let node = parse_node(value)?;
    Ok(AudioNoteCandidate {
        track: field_atom(&node, "track")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        onset_sample: field_atom(&node, "onset_sample")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        duration_samples: field_atom(&node, "duration_samples")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        sample_rate: field_atom(&node, "sample_rate")?
            .parse()
            .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        pitch: Pitch::from_semitone(
            field_atom(&node, "semitone")?
                .parse()
                .map_err(|_| SoundShapeError::InvalidSoundShape)?,
        ),
        mean_frequency: decode_frequency(&field_form_text(&node, "mean_frequency")?)?,
        mean_amplitude: decode_amplitude(&field_form_text(&node, "mean_amplitude")?)?,
        confidence: parse_f64(&field_atom(&node, "confidence")?)?,
        diagnostics: string_list(field_list(&node, "diagnostics")?)?,
    })
}

fn string_list(nodes: &[Node]) -> Result<Vec<String>, SoundShapeError> {
    nodes
        .iter()
        .map(|node| match node {
            Node::String(value) => Ok(value.clone()),
            _ => Err(SoundShapeError::InvalidSoundShape),
        })
        .collect()
}
