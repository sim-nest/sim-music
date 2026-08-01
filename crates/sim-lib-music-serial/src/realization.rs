//! Strict serial-plan realization output and failures.

use std::collections::BTreeMap;

use sim_lib_music_core::{Note, Time};
use sim_lib_pitch_serial::RowForm;
use thiserror::Error;

use crate::{OrdinalRef, SerialEventId, SerialPlan, StructuralLicense, VoiceId};

/// Complete serial provenance for one realized sounding note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedSerialOrigin {
    /// Structural readings that license the realized note.
    pub licenses: Vec<StructuralLicense>,
    /// Every structural ordinal cited by the planned event.
    pub ordinals: Vec<OrdinalRef>,
    /// The specific ordinal realized by this note.
    pub source_ordinal: OrdinalRef,
    /// Row forms keyed by the row instances referenced by `ordinals`.
    pub row_forms: BTreeMap<crate::RowInstanceId, RowForm>,
}

/// One realized sounding note with stable plan/event provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedSerialNote {
    /// Stable planned event identity.
    pub event_id: SerialEventId,
    /// Stable voice identity chosen for the note.
    pub voice: VoiceId,
    /// Stable ordinal occurrence within the event's rendered note list.
    pub note_index: usize,
    /// Exact absolute onset in whole-note units.
    pub onset: Time,
    /// Sounding note payload.
    pub note: Note,
    /// Serial provenance retained for this note.
    pub origin: RealizedSerialOrigin,
}

/// One realized event span, which may sound notes or occupy time as a rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedSerialEvent {
    /// Stable planned event identity.
    pub event_id: SerialEventId,
    /// Exact onset assigned during realization.
    pub onset: Time,
    /// Exact occupied duration, whether or not the event sounds notes.
    pub duration: Time,
    /// Whether this event occupies silence rather than sounding notes.
    pub is_rest: bool,
    /// Whether this event tied into the following same-voice event.
    pub ties_into_next: bool,
}

/// Realized serial notes plus exact event spans, retaining the source plan unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialRealization {
    plan: SerialPlan,
    events: Vec<RealizedSerialEvent>,
    notes: Vec<RealizedSerialNote>,
}

impl SerialRealization {
    /// Builds one exact realization from the preserved plan, event spans, and notes.
    pub fn new(
        plan: SerialPlan,
        mut events: Vec<RealizedSerialEvent>,
        mut notes: Vec<RealizedSerialNote>,
    ) -> Self {
        events.sort_by(|left, right| {
            left.onset
                .cmp(&right.onset)
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        notes.sort_by(|left, right| {
            left.onset
                .cmp(&right.onset)
                .then_with(|| left.voice.cmp(&right.voice))
                .then_with(|| left.note.pitch.cmp(&right.note.pitch))
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.note_index.cmp(&right.note_index))
        });
        Self {
            plan,
            events,
            notes,
        }
    }

    /// Returns the equality-identical structural source plan.
    pub fn plan(&self) -> &SerialPlan {
        &self.plan
    }

    /// Returns the exact realized event spans in canonical time order.
    pub fn events(&self) -> &[RealizedSerialEvent] {
        &self.events
    }

    /// Returns the realized sounding notes in canonical order.
    pub fn notes(&self) -> &[RealizedSerialNote] {
        &self.notes
    }
}

/// Failure while realizing or rendering a strict serial plan.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum StrictRealizationError {
    /// One plan event lacked an explicit realization spec.
    #[error("plan event {0} is missing a strict realization spec")]
    MissingSpec(SerialEventId),
    /// One event spec named an impossible register or octave displacement result.
    #[error("event {event_id} realizes MIDI pitch {midi}, outside 0..=127")]
    MidiOutOfRange {
        /// Affected event.
        event_id: SerialEventId,
        /// Rejected MIDI note number.
        midi: i16,
    },
    /// The caller supplied a non-positive duration.
    #[error("event {0} must have a strictly positive duration")]
    NonPositiveDuration(SerialEventId),
    /// The octave-displacement vector does not match the event's cardinality.
    #[error("event {event_id} has {ordinals} ordinals but {displacements} octave displacements")]
    OctaveDisplacementMismatch {
        /// Affected event.
        event_id: SerialEventId,
        /// Event ordinal count.
        ordinals: usize,
        /// Supplied displacement count.
        displacements: usize,
    },
    /// A tie requested a following same-voice event that did not exist.
    #[error("event {0} ties into the next event, but no later same-voice event exists")]
    MissingTieTarget(SerialEventId),
    /// A tie target did not realize the same pitch multiplicity.
    #[error("event {source_event} cannot tie into {target_event}: {reason}")]
    InvalidTieTarget {
        /// Source event requesting the tie.
        source_event: SerialEventId,
        /// Target event.
        target_event: SerialEventId,
        /// Human-readable mismatch reason.
        reason: &'static str,
    },
    /// Rendering through canonical music-core score conversion failed.
    #[error("music-core rendering failed: {0}")]
    MusicCore(String),
}
