//! Identity-bearing analysis events and shared transform evidence.

use sim_lib_music_core::{ObjectId, Pitch, Staff, Time};
use thiserror::Error;

/// One exact, identity-bearing note event accepted by sequence analyzers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisEvent {
    /// Identity of the source voice.
    pub voice_id: ObjectId,
    /// Logical note identity.
    pub note_id: ObjectId,
    /// Score-event identity.
    pub event_id: ObjectId,
    /// Exact absolute onset in whole-note units.
    pub onset: Time,
    /// Exact notated duration in whole-note units.
    pub duration: Time,
    /// Absolute pitch.
    pub pitch: Pitch,
}

/// Exact half-open span used by pattern reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeSpan {
    /// Inclusive span start.
    pub start: Time,
    /// Exclusive span end.
    pub end: Time,
}

impl TimeSpan {
    /// Builds a valid half-open span.
    pub fn new(start: Time, end: Time) -> Result<Self, AnalysisError> {
        if start < Time::from_integer(0) || end < start {
            return Err(AnalysisError::InvalidInput {
                field: "span",
                reason: "span must satisfy 0 <= start <= end".to_owned(),
            });
        }
        Ok(Self { start, end })
    }
}

/// Exact affine transform that maps one musical occurrence onto another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisTransform {
    /// Signed chromatic pitch shift in semitones.
    pub transposition: i32,
    /// Exact multiplier applied to onsets and durations.
    pub time_scale: Time,
    /// Exact offset applied after time scaling.
    pub time_shift: Time,
}

impl AnalysisTransform {
    /// Returns the identity transform.
    pub fn identity() -> Self {
        Self {
            transposition: 0,
            time_scale: Time::from_integer(1),
            time_shift: Time::from_integer(0),
        }
    }
}

/// Failure from music-domain analysis policy, admission, or delegated engines.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AnalysisError {
    /// An input value violated a named invariant.
    #[error("invalid {field}: {reason}")]
    InvalidInput {
        /// Invalid field.
        field: &'static str,
        /// Stable explanation.
        reason: String,
    },
    /// A declared policy value was invalid.
    #[error("invalid {field} policy: {reason}")]
    InvalidPolicy {
        /// Invalid policy field.
        field: &'static str,
        /// Stable explanation.
        reason: String,
    },
    /// A deterministic resource preflight rejected the request.
    #[error("{resource} requires {required}, exceeding {maximum}")]
    ResourceLimit {
        /// Bounded resource name.
        resource: &'static str,
        /// Required units.
        required: u64,
        /// Caller-declared ceiling.
        maximum: u64,
    },
    /// Generic discrete alignment failed closed.
    #[error("sequence alignment failed: {0}")]
    Alignment(String),
    /// Generic signal correlation failed closed.
    #[error("signal correlation failed: {0}")]
    Correlation(String),
    /// Rebuilding an identity-bearing staff failed.
    #[error("quantized staff was invalid: {0}")]
    Staff(String),
}

/// Projects a staff into stable time/pitch/event order without losing identity.
pub fn analysis_events(staff: &Staff) -> Vec<AnalysisEvent> {
    let mut events = staff
        .notes()
        .map(|note| AnalysisEvent {
            voice_id: note.voice_id.clone(),
            note_id: note.note_id.clone(),
            event_id: note.event_id.clone(),
            onset: note.onset,
            duration: note.note.duration,
            pitch: note.note.pitch,
        })
        .collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.onset
            .cmp(&right.onset)
            .then_with(|| left.pitch.cmp(&right.pitch))
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    events
}

pub(crate) fn ratio_to_f64(value: Time) -> Result<f64, AnalysisError> {
    let value = *value.numer() as f64 / *value.denom() as f64;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AnalysisError::InvalidInput {
            field: "time",
            reason: "exact time was outside finite f64 analysis range".to_owned(),
        })
    }
}

pub(crate) fn event_span(events: &[AnalysisEvent]) -> Result<TimeSpan, AnalysisError> {
    let start = events
        .first()
        .ok_or_else(|| AnalysisError::InvalidInput {
            field: "events",
            reason: "at least one event is required".to_owned(),
        })?
        .onset;
    let end = events
        .iter()
        .map(|event| event.onset + event.duration)
        .max()
        .expect("non-empty events");
    TimeSpan::new(start, end)
}

pub(crate) fn sequence_extent(events: &[AnalysisEvent]) -> Time {
    let onset_extent = events.last().expect("non-empty").onset - events[0].onset;
    if onset_extent > Time::from_integer(0) {
        onset_extent
    } else {
        events
            .iter()
            .map(|event| event.duration)
            .max()
            .unwrap_or_else(|| Time::from_integer(1))
    }
}
