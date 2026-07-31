use std::collections::BTreeSet;

use sim_lib_music_core::{Staff, StaffNote, StaffVoice};

use super::{MusicTransform, MusicTransformChange, finish};
use crate::TransformError;

/// Material introduced by a strictly additive staff transform.
///
/// New voices carry their complete initial contents. `notes` contains notes
/// appended to an existing voice or to a voice introduced by this patch.
/// Applying the patch never changes existing voice metadata or note payloads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdditiveStaffPatch {
    /// Complete voices introduced by the patch.
    pub voices: Vec<StaffVoice>,
    /// Notes introduced into named voices.
    pub notes: Vec<StaffNote>,
}

/// Applies new voices and notes while retaining every source value verbatim.
pub fn apply_additive_staff_patch(
    source: &Staff,
    patch: &AdditiveStaffPatch,
) -> Result<MusicTransform<Staff>, TransformError> {
    let source_ids = source.object_ids().into_iter().collect::<BTreeSet<_>>();
    let mut voices = source.voices.clone();
    let mut changes = Vec::new();

    for voice in &patch.voices {
        if source_ids.contains(&voice.id) || voices.iter().any(|item| item.id == voice.id) {
            return invalid("an added voice identity already exists");
        }
        changes.push(MusicTransformChange::AddedVoice {
            voice_id: voice.id.clone(),
        });
        for note in &voice.notes {
            changes.push(added_note_change(note));
        }
        voices.push(voice.clone());
    }

    for note in &patch.notes {
        let Some(voice) = voices.iter_mut().find(|voice| voice.id == note.voice_id) else {
            return invalid("an added note names a missing voice");
        };
        changes.push(added_note_change(note));
        voice.notes.push(note.clone());
    }

    let transformed = finish(voices, changes)?;
    ensure_source_unchanged(source, &transformed.value)?;
    Ok(transformed)
}

/// Removes exactly the material named by an additive patch.
///
/// Removal fails closed if an introduced note or voice has been changed,
/// removed, or replaced since application. This prevents the inverse from
/// deleting material merely because it reuses an identity.
pub fn remove_additive_staff_patch(
    completed: &Staff,
    patch: &AdditiveStaffPatch,
) -> Result<MusicTransform<Staff>, TransformError> {
    let mut voices = completed.voices.clone();
    let mut changes = Vec::new();

    for note in patch.notes.iter().rev() {
        let Some(voice) = voices.iter_mut().find(|voice| voice.id == note.voice_id) else {
            return invalid("the completed staff is missing an added note voice");
        };
        let Some(index) = voice.notes.iter().position(|candidate| candidate == note) else {
            return invalid("an added note is missing or has changed");
        };
        voice.notes.remove(index);
        changes.push(MusicTransformChange::Removed {
            note_id: note.note_id.clone(),
            event_id: note.event_id.clone(),
            reason: "reverse additive staff patch",
        });
    }

    for added in patch.voices.iter().rev() {
        let Some(index) = voices.iter().position(|voice| voice.id == added.id) else {
            return invalid("an added voice is missing");
        };
        if voices[index] != *added {
            return invalid("an added voice has changed");
        }
        for note in &added.notes {
            changes.push(MusicTransformChange::Removed {
                note_id: note.note_id.clone(),
                event_id: note.event_id.clone(),
                reason: "reverse additive staff patch",
            });
        }
        changes.push(MusicTransformChange::RemovedVoice {
            voice_id: added.id.clone(),
        });
        voices.remove(index);
    }

    finish(voices, changes)
}

fn added_note_change(note: &StaffNote) -> MusicTransformChange {
    MusicTransformChange::AddedNote {
        voice_id: note.voice_id.clone(),
        note_id: note.note_id.clone(),
        event_id: note.event_id.clone(),
    }
}

fn ensure_source_unchanged(source: &Staff, completed: &Staff) -> Result<(), TransformError> {
    for source_voice in &source.voices {
        let Some(completed_voice) = completed
            .voices
            .iter()
            .find(|voice| voice.id == source_voice.id)
        else {
            return invalid("an additive transform removed a source voice");
        };
        if completed_voice.name != source_voice.name
            || completed_voice.duration != source_voice.duration
            || !source_voice
                .notes
                .iter()
                .all(|note| completed_voice.notes.contains(note))
        {
            return invalid("an additive transform changed source material");
        }
    }
    Ok(())
}

fn invalid<T>(reason: &'static str) -> Result<T, TransformError> {
    Err(TransformError::InvalidTransformOutput {
        transform: "additive-staff-patch",
        reason,
    })
}
