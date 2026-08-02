use std::collections::BTreeMap;

use sim_lib_music_core::{Pitch, Staff};

use super::{MusicTransform, MusicTransformChange, finish, note_order, transform_notes};
use crate::TransformError;

/// Tie direction for equally near octave placements.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegisterTie {
    /// Select the higher candidate.
    Ascending,
    /// Select the lower candidate.
    Descending,
}

/// Inclusive semitone bounds and tie policy for register unwrapping.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RegisterRange {
    /// Lowest allowed pitch.
    pub low: Pitch,
    /// Highest allowed pitch.
    pub high: Pitch,
    /// Direction chosen for equally near candidates.
    pub tie: RegisterTie,
}

/// Chooses octave placements that minimize adjacent motion within `range`.
///
/// Pitch classes, timings, voices, and identities remain fixed. The report
/// stores both pitches for every register edit, making [`restore_register`]
/// an exact inverse.
pub fn unwrap_register(
    staff: &Staff,
    range: RegisterRange,
) -> Result<MusicTransform<Staff>, TransformError> {
    let low = range.low.semitone();
    let high = range.high.semitone();
    if low > high {
        return Err(TransformError::InvalidTransformOutput {
            transform: "register-unwrap",
            reason: "low pitch must not exceed high pitch",
        });
    }
    let mut voices = staff.voices.clone();
    let mut changes = Vec::new();
    for voice in &mut voices {
        voice.notes.sort_by(note_order);
        let mut previous = None;
        for note in &mut voice.notes {
            let original = note.note.pitch;
            let target = previous.unwrap_or_else(|| original.semitone());
            let candidates = register_candidates(original, low, high);
            let Some(semitone) = nearest_candidate(&candidates, target, range.tie) else {
                return Err(TransformError::InvalidTransformOutput {
                    transform: "register-unwrap",
                    reason: "register contains no octave placement for a pitch class",
                });
            };
            note.note.pitch = Pitch::from_semitone(semitone);
            previous = Some(semitone);
            if note.note.pitch != original {
                changes.push(MusicTransformChange::Pitch {
                    event_id: note.event_id.clone(),
                    before: original,
                    after: note.note.pitch,
                });
            }
        }
    }
    finish(voices, changes)
}

/// Restores the original pitches recorded by [`unwrap_register`].
pub fn restore_register(
    report: &MusicTransform<Staff>,
) -> Result<MusicTransform<Staff>, TransformError> {
    let originals = report
        .changes
        .iter()
        .filter_map(|change| match change {
            MusicTransformChange::Pitch {
                event_id, before, ..
            } => Some((event_id.clone(), *before)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    transform_notes(&report.value, |mut note, changes| {
        if let Some(original) = originals.get(&note.event_id)
            && note.note.pitch != *original
        {
            let before = note.note.pitch;
            note.note.pitch = *original;
            changes.push(MusicTransformChange::Pitch {
                event_id: note.event_id.clone(),
                before,
                after: *original,
            });
        }
        note
    })
}

fn register_candidates(pitch: Pitch, low: i32, high: i32) -> Vec<i32> {
    let class = i32::from(pitch.class.value());
    let mut candidate = low + (class - low).rem_euclid(12);
    let mut output = Vec::new();
    while candidate <= high {
        output.push(candidate);
        candidate += 12;
    }
    output
}

fn nearest_candidate(candidates: &[i32], target: i32, tie: RegisterTie) -> Option<i32> {
    candidates.iter().copied().min_by(|left, right| {
        let left_distance = (*left - target).abs();
        let right_distance = (*right - target).abs();
        left_distance.cmp(&right_distance).then_with(|| match tie {
            RegisterTie::Ascending => right.cmp(left),
            RegisterTie::Descending => left.cmp(right),
        })
    })
}
