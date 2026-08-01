//! Canonical music-core rendering for serial realizations.

use std::collections::BTreeMap;

use sim_lib_music_core::{
    AmbiguousConversionPolicy, Music, ObjectId, PianoRoll, Score, ScoreForm, ScoreFormKind, Staff,
    StaffNote, StaffVoice, convert_score,
};

use crate::{SerialRealization, StrictRealizationError};

/// Score metadata used when wrapping a rendered serial realization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialRenderOptions {
    /// Tempo in beats per minute.
    pub tempo_bpm: u32,
    /// Time signature numerator and denominator.
    pub time_signature: (u8, u8),
    /// Optional key label.
    pub key: Option<String>,
}

impl Default for SerialRenderOptions {
    fn default() -> Self {
        Self {
            tempo_bpm: 60,
            time_signature: (4, 4),
            key: None,
        }
    }
}

/// Renders one realization to the canonical identity-bearing [`Staff`].
pub fn render_serial_staff(
    realization: &SerialRealization,
) -> Result<Staff, StrictRealizationError> {
    let mut voices = BTreeMap::<_, StaffVoice>::new();
    for event in realization.events() {
        let planned = realization
            .plan()
            .event(&event.event_id)
            .expect("realization events must reference plan events");
        let voice = voices
            .entry(planned.voice.clone())
            .or_insert_with(|| StaffVoice {
                id: planned.voice.clone(),
                name: planned.voice.as_str().to_owned(),
                duration: event.onset + event.duration,
                notes: Vec::new(),
            });
        voice.duration = voice.duration.max(event.onset + event.duration);
    }
    for note in realization.notes() {
        let voice = voices
            .entry(note.voice.clone())
            .or_insert_with(|| StaffVoice {
                id: note.voice.clone(),
                name: note.voice.as_str().to_owned(),
                duration: note.onset + note.note.duration,
                notes: Vec::new(),
            });
        voice.duration = voice.duration.max(note.onset + note.note.duration);
        voice.notes.push(StaffNote {
            voice_id: note.voice.clone(),
            note_id: ObjectId::new(format!(
                "serial-note/{}/{}/{}",
                note.event_id, note.note_index, note.origin.source_ordinal.ordinal
            ))
            .map_err(|error| StrictRealizationError::MusicCore(error.to_string()))?,
            event_id: ObjectId::new(format!(
                "serial-event/{}/{}",
                note.event_id, note.note_index
            ))
            .map_err(|error| StrictRealizationError::MusicCore(error.to_string()))?,
            onset: note.onset,
            note: note.note.clone(),
        });
    }
    Staff::new(voices.into_values().collect())
        .map_err(|error| StrictRealizationError::MusicCore(error.to_string()))
}

/// Renders one realization to the canonical [`PianoRoll`] through [`Staff`].
pub fn render_serial_piano_roll(
    realization: &SerialRealization,
) -> Result<PianoRoll, StrictRealizationError> {
    let staff = render_serial_staff(realization)?;
    let report = convert_score(
        &ScoreForm::Staff(staff),
        ScoreFormKind::PianoRoll,
        AmbiguousConversionPolicy::Reject,
    )
    .map_err(|error| StrictRealizationError::MusicCore(error.to_string()))?;
    let ScoreForm::PianoRoll(roll) = report.value else {
        unreachable!("piano-roll conversion must return a piano roll");
    };
    Ok(roll)
}

/// Wraps the canonical piano roll in a music-core [`Score`].
pub fn render_serial_score(
    realization: &SerialRealization,
    options: &SerialRenderOptions,
) -> Result<Score, StrictRealizationError> {
    Score::new(
        options.tempo_bpm,
        options.time_signature,
        options.key.clone(),
        Music::PianoRoll(render_serial_piano_roll(realization)?),
    )
    .map_err(|error| StrictRealizationError::MusicCore(error.to_string()))
}
