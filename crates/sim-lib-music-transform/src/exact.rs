//! Exact identity-preserving staff transforms and their audit reports.

mod composition;
mod register;

use std::collections::BTreeSet;

use sim_lib_music_core::{
    Articulation, Channel, ObjectId, Pitch, Staff, StaffNote, StaffVoice, Time,
};

use crate::TransformError;

pub use composition::*;
pub use register::*;

/// One reversible or explicitly destructive change made by an exact transform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusicTransformChange {
    /// A note onset changed.
    Onset {
        /// Affected event.
        event_id: ObjectId,
        /// Exact prior onset.
        before: Time,
        /// Exact new onset.
        after: Time,
    },
    /// A note duration changed.
    Duration {
        /// Affected event.
        event_id: ObjectId,
        /// Exact prior duration.
        before: Time,
        /// Exact new duration.
        after: Time,
    },
    /// A note articulation changed.
    Articulation {
        /// Affected event.
        event_id: ObjectId,
        /// Prior articulation.
        before: Articulation,
        /// New articulation.
        after: Articulation,
    },
    /// A note pitch changed while its pitch class stayed fixed.
    Pitch {
        /// Affected event.
        event_id: ObjectId,
        /// Prior pitch.
        before: Pitch,
        /// New pitch.
        after: Pitch,
    },
    /// A note moved to another voice.
    Voice {
        /// Affected event.
        event_id: ObjectId,
        /// Prior voice identity.
        before: ObjectId,
        /// New voice identity.
        after: ObjectId,
    },
    /// Voice separation allocated a new voice identity.
    CreatedVoice {
        /// New identity.
        voice_id: ObjectId,
        /// Original voice from which it was split.
        source_voice_id: ObjectId,
    },
    /// A rhythm mask or slice removed an event.
    Removed {
        /// Removed logical note identity.
        note_id: ObjectId,
        /// Removed event identity.
        event_id: ObjectId,
        /// Stable reason for removal.
        reason: &'static str,
    },
}

/// Exact transform value paired with identity and change evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicTransform<T> {
    /// Transformed value.
    pub value: T,
    /// Identities still present after the transform.
    pub preserved: Vec<ObjectId>,
    /// Complete ordered edits performed by the transform.
    pub changes: Vec<MusicTransformChange>,
}

impl<T> MusicTransform<T> {
    /// Returns `true` when the transform left its input unchanged.
    pub fn is_unchanged(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Exact half-open sustain-pedal interval.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SustainSpan {
    /// Pedal-down time.
    pub start: Time,
    /// Pedal-up time.
    pub end: Time,
    /// Optional channel restriction.
    pub channel: Option<Channel>,
}

impl SustainSpan {
    /// Builds a sustain span; validity is checked when applying it.
    pub fn new(start: Time, end: Time, channel: Option<Channel>) -> Self {
        Self {
            start,
            end,
            channel,
        }
    }
}

/// Ordering used for simultaneous notes during delayed-note voice separation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DelayedNoteOrder {
    /// Stable pitch/event identity order.
    Stable,
    /// Higher pitches receive earlier voice slots.
    HighestFirst,
    /// Lower pitches receive earlier voice slots.
    LowestFirst,
}

/// Periodic exact-onset rhythm mask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RhythmMask {
    step: Time,
    pattern: Vec<bool>,
}

impl RhythmMask {
    /// Builds a non-empty mask with a positive exact step.
    pub fn new(step: Time, pattern: Vec<bool>) -> Result<Self, TransformError> {
        if step <= Time::from_integer(0) {
            return Err(TransformError::InvalidFactor);
        }
        if pattern.is_empty() {
            return Err(TransformError::InvalidTransformOutput {
                transform: "rhythm-mask",
                reason: "pattern must not be empty",
            });
        }
        Ok(Self { step, pattern })
    }

    /// Returns the exact duration of one mask slot.
    pub fn step(&self) -> Time {
        self.step
    }

    /// Returns the periodic keep/drop pattern.
    pub fn pattern(&self) -> &[bool] {
        &self.pattern
    }

    fn keeps(&self, onset: Time) -> bool {
        let slots = onset / self.step;
        let slot = slots.numer().div_euclid(*slots.denom());
        self.pattern[slot.rem_euclid(self.pattern.len() as i64) as usize]
    }
}

/// Extends note releases that occur while one of `spans` is active.
///
/// Onsets, pitches, identities, and exact rational time are retained. A note
/// released inside overlapping sustain spans is extended through the furthest
/// applicable pedal-up boundary.
pub fn sustain_staff(
    staff: &Staff,
    spans: &[SustainSpan],
) -> Result<MusicTransform<Staff>, TransformError> {
    validate_spans(spans)?;
    transform_notes(staff, |mut note, changes| {
        let before = note.note.duration;
        let mut end = note.end();
        for span in spans {
            if span
                .channel
                .is_none_or(|channel| channel == note.note.channel)
                && end >= span.start
                && end < span.end
                && note.onset < span.end
            {
                end = span.end;
            }
        }
        note.note.duration = end - note.onset;
        if note.note.duration != before {
            changes.push(MusicTransformChange::Duration {
                event_id: note.event_id.clone(),
                before,
                after: note.note.duration,
            });
        }
        note
    })
}

/// Connects each note in a voice to its next onset and marks it legato.
///
/// Existing overlaps are not shortened. The final note of each voice is left
/// unchanged because there is no following articulation target.
pub fn slur_staff(staff: &Staff) -> Result<MusicTransform<Staff>, TransformError> {
    let mut voices = staff.voices.clone();
    let mut changes = Vec::new();
    for voice in &mut voices {
        voice.notes.sort_by(note_order);
        for index in 0..voice.notes.len().saturating_sub(1) {
            let next_onset = voice.notes[index + 1].onset;
            let note = &mut voice.notes[index];
            if note.end() < next_onset {
                let before = note.note.duration;
                note.note.duration = next_onset - note.onset;
                changes.push(MusicTransformChange::Duration {
                    event_id: note.event_id.clone(),
                    before,
                    after: note.note.duration,
                });
            }
            if note.note.articulation != Articulation::Legato {
                let before = note.note.articulation;
                note.note.articulation = Articulation::Legato;
                changes.push(MusicTransformChange::Articulation {
                    event_id: note.event_id.clone(),
                    before,
                    after: Articulation::Legato,
                });
            }
        }
    }
    finish(voices, changes)
}

/// Expands every onset, note duration, and voice span by a positive exact factor.
pub fn expand_staff(staff: &Staff, factor: Time) -> Result<MusicTransform<Staff>, TransformError> {
    if factor <= Time::from_integer(0) {
        return Err(TransformError::InvalidFactor);
    }
    let mut voices = staff.voices.clone();
    let mut changes = Vec::new();
    for voice in &mut voices {
        voice.duration *= factor;
        for note in &mut voice.notes {
            let onset = note.onset;
            let duration = note.note.duration;
            note.onset *= factor;
            note.note.duration *= factor;
            if note.onset != onset {
                changes.push(MusicTransformChange::Onset {
                    event_id: note.event_id.clone(),
                    before: onset,
                    after: note.onset,
                });
            }
            if note.note.duration != duration {
                changes.push(MusicTransformChange::Duration {
                    event_id: note.event_id.clone(),
                    before: duration,
                    after: note.note.duration,
                });
            }
        }
    }
    finish(voices, changes)
}

/// Splits delayed overlapping notes into monophonic voices without moving them.
///
/// Exact abutment (`previous.end == next.onset`) remains in one voice. The first
/// output retains the original voice id; additional lines receive deterministic
/// derived ids, while every note/event identity is preserved.
pub fn separate_delayed_notes(
    staff: &Staff,
    order: DelayedNoteOrder,
) -> Result<MusicTransform<Staff>, TransformError> {
    let mut output = Vec::new();
    let mut changes = Vec::new();
    for voice in &staff.voices {
        let mut notes = voice.notes.clone();
        notes.sort_by(|left, right| delayed_order(left, right, order));
        let mut lines = Vec::<StaffVoice>::new();
        for mut note in notes {
            let slot = lines.iter().position(|line| {
                line.notes
                    .last()
                    .is_none_or(|last| last.end() <= note.onset)
            });
            let index = slot.unwrap_or(lines.len());
            if index == lines.len() {
                let id = if index == 0 {
                    voice.id.clone()
                } else {
                    ObjectId::new(format!("{}/delayed-{index}", voice.id))
                        .expect("derived voice identity is non-empty")
                };
                if index > 0 {
                    changes.push(MusicTransformChange::CreatedVoice {
                        voice_id: id.clone(),
                        source_voice_id: voice.id.clone(),
                    });
                }
                lines.push(StaffVoice {
                    id,
                    name: if index == 0 {
                        voice.name.clone()
                    } else {
                        format!("{} delayed {}", voice.name, index + 1)
                    },
                    duration: voice.duration,
                    notes: Vec::new(),
                });
            }
            let destination = lines[index].id.clone();
            if note.voice_id != destination {
                changes.push(MusicTransformChange::Voice {
                    event_id: note.event_id.clone(),
                    before: note.voice_id.clone(),
                    after: destination.clone(),
                });
                note.voice_id = destination;
            }
            lines[index].notes.push(note);
        }
        if lines.is_empty() {
            lines.push(voice.clone());
        }
        output.extend(lines);
    }
    finish(output, changes)
}

fn transform_notes(
    staff: &Staff,
    mut f: impl FnMut(StaffNote, &mut Vec<MusicTransformChange>) -> StaffNote,
) -> Result<MusicTransform<Staff>, TransformError> {
    let mut voices = staff.voices.clone();
    let mut changes = Vec::new();
    for voice in &mut voices {
        voice.notes = voice
            .notes
            .drain(..)
            .map(|note| f(note, &mut changes))
            .collect();
        if let Some(end) = voice.notes.iter().map(StaffNote::end).max() {
            voice.duration = voice.duration.max(end);
        }
    }
    finish(voices, changes)
}

fn finish(
    voices: Vec<StaffVoice>,
    changes: Vec<MusicTransformChange>,
) -> Result<MusicTransform<Staff>, TransformError> {
    let staff = Staff::new(voices).map_err(TransformError::InvalidStaff)?;
    let created = changes
        .iter()
        .filter_map(|change| match change {
            MusicTransformChange::CreatedVoice { voice_id, .. } => Some(voice_id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    Ok(MusicTransform {
        preserved: staff
            .object_ids()
            .into_iter()
            .filter(|id| !created.contains(id))
            .collect(),
        value: staff,
        changes,
    })
}

fn validate_spans(spans: &[SustainSpan]) -> Result<(), TransformError> {
    if spans
        .iter()
        .any(|span| span.start < Time::from_integer(0) || span.end < span.start)
    {
        return Err(TransformError::InvalidTransformOutput {
            transform: "sustain",
            reason: "sustain spans must satisfy 0 <= start <= end",
        });
    }
    Ok(())
}

fn note_order(left: &StaffNote, right: &StaffNote) -> std::cmp::Ordering {
    left.onset
        .cmp(&right.onset)
        .then_with(|| left.note.pitch.cmp(&right.note.pitch))
        .then_with(|| left.event_id.cmp(&right.event_id))
}

fn delayed_order(
    left: &StaffNote,
    right: &StaffNote,
    order: DelayedNoteOrder,
) -> std::cmp::Ordering {
    left.onset.cmp(&right.onset).then_with(|| {
        let pitch = left.note.pitch.cmp(&right.note.pitch);
        let pitch = match order {
            DelayedNoteOrder::Stable | DelayedNoteOrder::LowestFirst => pitch,
            DelayedNoteOrder::HighestFirst => pitch.reverse(),
        };
        pitch.then_with(|| left.event_id.cmp(&right.event_id))
    })
}
