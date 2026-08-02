//! Local reversible additive staff patching retained inside serial.

use std::collections::BTreeSet;

use sim_lib_music_core::{Staff, StaffNote, StaffVoice};

/// Material introduced by a strictly additive staff transform.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdditiveStaffPatch {
    /// Complete voices introduced by the patch.
    pub voices: Vec<StaffVoice>,
    /// Notes introduced into named voices.
    pub notes: Vec<StaffNote>,
}

pub(crate) fn apply_additive_staff_patch(
    source: &Staff,
    patch: &AdditiveStaffPatch,
) -> Result<Staff, String> {
    let source_ids = source.object_ids().into_iter().collect::<BTreeSet<_>>();
    let mut voices = source.voices.clone();

    for voice in &patch.voices {
        if source_ids.contains(&voice.id) || voices.iter().any(|item| item.id == voice.id) {
            return Err("an added voice identity already exists".to_owned());
        }
        voices.push(voice.clone());
    }

    for note in &patch.notes {
        let Some(voice) = voices.iter_mut().find(|voice| voice.id == note.voice_id) else {
            return Err("an added note names a missing voice".to_owned());
        };
        voice.notes.push(note.clone());
    }

    let completed = Staff::new(voices).map_err(|err| err.to_string())?;
    ensure_source_unchanged(source, &completed)?;
    Ok(completed)
}

pub(crate) fn remove_additive_staff_patch(
    completed: &Staff,
    patch: &AdditiveStaffPatch,
) -> Result<Staff, String> {
    let mut voices = completed.voices.clone();

    for note in patch.notes.iter().rev() {
        let Some(voice) = voices.iter_mut().find(|voice| voice.id == note.voice_id) else {
            return Err("the completed staff is missing an added note voice".to_owned());
        };
        let Some(index) = voice.notes.iter().position(|candidate| candidate == note) else {
            return Err("an added note is missing or has changed".to_owned());
        };
        voice.notes.remove(index);
    }

    for added in patch.voices.iter().rev() {
        let Some(index) = voices.iter().position(|voice| voice.id == added.id) else {
            return Err("an added voice is missing".to_owned());
        };
        if voices[index] != *added {
            return Err("an added voice has changed".to_owned());
        }
        voices.remove(index);
    }

    Staff::new(voices).map_err(|err| err.to_string())
}

fn ensure_source_unchanged(source: &Staff, completed: &Staff) -> Result<(), String> {
    for source_voice in &source.voices {
        let Some(completed_voice) = completed
            .voices
            .iter()
            .find(|voice| voice.id == source_voice.id)
        else {
            return Err("an additive transform removed a source voice".to_owned());
        };
        if completed_voice.name != source_voice.name
            || completed_voice.duration != source_voice.duration
            || !source_voice
                .notes
                .iter()
                .all(|note| completed_voice.notes.contains(note))
        {
            return Err("an additive transform changed source material".to_owned());
        }
    }
    Ok(())
}
