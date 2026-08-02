use std::collections::{BTreeMap, BTreeSet};

use sim_lib_music_core::{ObjectId, Staff, StaffNote, StaffVoice, Time};

use super::{MusicTransform, MusicTransformChange, RhythmMask, finish};
use crate::TransformError;

/// Composes staffs in sequence, shifting later onsets by exact prior durations.
///
/// Matching voice ids are joined. Note and event identities must be globally
/// distinct, because silently renaming them would violate identity preservation.
pub fn sequence_staff(parts: &[Staff]) -> Result<MusicTransform<Staff>, TransformError> {
    let mut voices = BTreeMap::<ObjectId, StaffVoice>::new();
    let mut seen = BTreeSet::new();
    let mut offset = Time::from_integer(0);
    let mut changes = Vec::new();
    for staff in parts {
        for voice in &staff.voices {
            let output = voices
                .entry(voice.id.clone())
                .or_insert_with(|| StaffVoice {
                    id: voice.id.clone(),
                    name: voice.name.clone(),
                    duration: Time::from_integer(0),
                    notes: Vec::new(),
                });
            if output.name != voice.name {
                return Err(TransformError::InvalidTransformOutput {
                    transform: "sequence-staff",
                    reason: "matching voice ids have different names",
                });
            }
            for note in &voice.notes {
                require_unique_note(note, &mut seen, "sequence-staff")?;
                let mut shifted = note.clone();
                shifted.onset += offset;
                if shifted.onset != note.onset {
                    changes.push(MusicTransformChange::Onset {
                        event_id: note.event_id.clone(),
                        before: note.onset,
                        after: shifted.onset,
                    });
                }
                output.notes.push(shifted);
            }
        }
        offset += staff.duration();
    }
    for voice in voices.values_mut() {
        voice.duration = offset;
    }
    finish(voices.into_values().collect(), changes)
}

/// Composes staffs in parallel, retaining exact onsets and maximum duration.
pub fn parallel_staff(parts: &[Staff]) -> Result<MusicTransform<Staff>, TransformError> {
    let mut voices = BTreeMap::<ObjectId, StaffVoice>::new();
    let mut seen = BTreeSet::new();
    for staff in parts {
        for voice in &staff.voices {
            let output = voices
                .entry(voice.id.clone())
                .or_insert_with(|| StaffVoice {
                    id: voice.id.clone(),
                    name: voice.name.clone(),
                    duration: Time::from_integer(0),
                    notes: Vec::new(),
                });
            if output.name != voice.name {
                return Err(TransformError::InvalidTransformOutput {
                    transform: "parallel-staff",
                    reason: "matching voice ids have different names",
                });
            }
            output.duration = output.duration.max(voice.duration);
            for note in &voice.notes {
                require_unique_note(note, &mut seen, "parallel-staff")?;
                output.notes.push(note.clone());
            }
        }
    }
    finish(voices.into_values().collect(), Vec::new())
}

/// Extracts an exact half-open staff slice and reports clipping/removal.
pub fn slice_staff(
    staff: &Staff,
    start: Time,
    end: Time,
) -> Result<MusicTransform<Staff>, TransformError> {
    if start < Time::from_integer(0) || end < start {
        return Err(TransformError::InvalidTransformOutput {
            transform: "slice-staff",
            reason: "slice must satisfy 0 <= start <= end",
        });
    }
    let duration = end - start;
    let mut voices = Vec::new();
    let mut changes = Vec::new();
    for voice in &staff.voices {
        let mut notes = Vec::new();
        for note in &voice.notes {
            let clipped_start = note.onset.max(start);
            let clipped_end = note.end().min(end);
            if clipped_start >= clipped_end {
                changes.push(MusicTransformChange::Removed {
                    note_id: note.note_id.clone(),
                    event_id: note.event_id.clone(),
                    reason: "outside slice",
                });
                continue;
            }
            let mut clipped = note.clone();
            clipped.onset = clipped_start - start;
            clipped.note.duration = clipped_end - clipped_start;
            if clipped.onset != note.onset {
                changes.push(MusicTransformChange::Onset {
                    event_id: note.event_id.clone(),
                    before: note.onset,
                    after: clipped.onset,
                });
            }
            if clipped.note.duration != note.note.duration {
                changes.push(MusicTransformChange::Duration {
                    event_id: note.event_id.clone(),
                    before: note.note.duration,
                    after: clipped.note.duration,
                });
            }
            notes.push(clipped);
        }
        voices.push(StaffVoice {
            id: voice.id.clone(),
            name: voice.name.clone(),
            duration,
            notes,
        });
    }
    finish(voices, changes)
}

/// Keeps notes whose exact onset lands on a `true` periodic mask slot.
pub fn apply_rhythm_mask(
    staff: &Staff,
    mask: &RhythmMask,
) -> Result<MusicTransform<Staff>, TransformError> {
    let mut voices = staff.voices.clone();
    let mut changes = Vec::new();
    for voice in &mut voices {
        voice.notes.retain(|note| {
            if mask.keeps(note.onset) {
                true
            } else {
                changes.push(MusicTransformChange::Removed {
                    note_id: note.note_id.clone(),
                    event_id: note.event_id.clone(),
                    reason: "rhythm mask",
                });
                false
            }
        });
    }
    finish(voices, changes)
}

fn require_unique_note(
    note: &StaffNote,
    seen: &mut BTreeSet<ObjectId>,
    transform: &'static str,
) -> Result<(), TransformError> {
    if !seen.insert(note.note_id.clone()) || !seen.insert(note.event_id.clone()) {
        return Err(TransformError::InvalidTransformOutput {
            transform,
            reason: "duplicate note or event identity",
        });
    }
    Ok(())
}
