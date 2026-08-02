//! Exact progression algebra over identity-bearing staffs.

use std::collections::BTreeMap;

use sim_lib_music_core::{ObjectId, Staff, StaffVoice, Time};

use super::{
    MusicTransform, MusicTransformChange, expand_staff, finish, parallel_staff, slice_staff,
};
use crate::TransformError;

/// Multiplies every exact onset, duration, and trailing span in a progression.
///
/// This is duration multiplication, not repetition: every object identity is
/// retained exactly once.
pub fn progression_multiply(
    progression: &Staff,
    factor: Time,
) -> Result<MusicTransform<Staff>, TransformError> {
    expand_staff(progression, factor)
}

/// Overlays exact progressions at the same origin.
///
/// The input identity sets must be disjoint except that equal voice ids with
/// equal names may be merged. Reusing a note or event id fails closed.
pub fn progression_overlay(
    progressions: &[Staff],
) -> Result<MusicTransform<Staff>, TransformError> {
    parallel_staff(progressions)
}

/// Extracts an exact half-open progression window.
pub fn progression_slice(
    progression: &Staff,
    start: Time,
    end: Time,
) -> Result<MusicTransform<Staff>, TransformError> {
    slice_staff(progression, start, end)
}

/// Repeats a progression back to back while keeping global identity unique.
///
/// The first occurrence preserves every original id. Later occurrences derive
/// note and event ids by appending `/repeat/<occurrence>` and report every
/// derivation as [`MusicTransformChange::RepeatedIdentity`]. Voice ids remain
/// stable because each repeated line is still the same logical voice.
pub fn progression_repeat(
    progression: &Staff,
    occurrences: usize,
) -> Result<MusicTransform<Staff>, TransformError> {
    if occurrences == 0 {
        return finish(Vec::new(), Vec::new());
    }
    let occurrence_count =
        i64::try_from(occurrences).map_err(|_| TransformError::InvalidTransformOutput {
            transform: "progression-repeat",
            reason: "occurrence count exceeds exact time range",
        })?;
    let span = progression.duration();
    let total_duration = span * Time::from_integer(occurrence_count);
    let mut voices = BTreeMap::<ObjectId, StaffVoice>::new();
    let mut changes = Vec::new();

    for occurrence in 0..occurrences {
        let occurrence_time =
            i64::try_from(occurrence).expect("occurrence was bounded by occurrence count");
        let offset = span * Time::from_integer(occurrence_time);
        for voice in &progression.voices {
            let output = voices
                .entry(voice.id.clone())
                .or_insert_with(|| StaffVoice {
                    id: voice.id.clone(),
                    name: voice.name.clone(),
                    duration: total_duration,
                    notes: Vec::new(),
                });
            if output.name != voice.name {
                return Err(TransformError::InvalidTransformOutput {
                    transform: "progression-repeat",
                    reason: "matching voice ids have different names",
                });
            }
            for source in &voice.notes {
                let mut repeated = source.clone();
                repeated.onset += offset;
                if occurrence > 0 {
                    repeated.note_id =
                        repeated_id(&source.note_id, occurrence, "progression-repeat")?;
                    repeated.event_id =
                        repeated_id(&source.event_id, occurrence, "progression-repeat")?;
                    changes.push(MusicTransformChange::RepeatedIdentity {
                        source_note_id: source.note_id.clone(),
                        source_event_id: source.event_id.clone(),
                        repeated_note_id: repeated.note_id.clone(),
                        repeated_event_id: repeated.event_id.clone(),
                        occurrence,
                    });
                }
                if repeated.onset != source.onset {
                    changes.push(MusicTransformChange::Onset {
                        event_id: repeated.event_id.clone(),
                        before: source.onset,
                        after: repeated.onset,
                    });
                }
                output.notes.push(repeated);
            }
        }
    }
    finish(voices.into_values().collect(), changes)
}

fn repeated_id(
    source: &ObjectId,
    occurrence: usize,
    transform: &'static str,
) -> Result<ObjectId, TransformError> {
    ObjectId::new(format!("{source}/repeat/{occurrence}")).map_err(|_| {
        TransformError::InvalidTransformOutput {
            transform,
            reason: "could not derive a repeated object identity",
        }
    })
}
