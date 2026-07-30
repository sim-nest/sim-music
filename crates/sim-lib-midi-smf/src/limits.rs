#![forbid(unsafe_code)]

use crate::{SmfError, SmfLimitKind};

/// Defensive resource bounds applied while parsing an SMF byte slice.
///
/// Limits are checked before capacity is reserved or a payload is copied.
/// [`read_smf`](crate::read_smf) uses [`Default`] limits; hosts accepting
/// untrusted files can choose tighter limits through
/// [`read_smf_with_limits`](crate::read_smf_with_limits).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SmfReadLimits {
    /// Maximum complete input size.
    pub max_file_bytes: usize,
    /// Maximum declared header chunk body size.
    pub max_header_bytes: usize,
    /// Maximum declared number of tracks.
    pub max_tracks: usize,
    /// Maximum body size of one track chunk.
    pub max_track_bytes: usize,
    /// Maximum total number of events across all tracks.
    pub max_events: usize,
    /// Maximum payload size of one meta or system-exclusive event.
    pub max_event_payload_bytes: usize,
    /// Maximum aggregate payload bytes copied across all events.
    pub max_total_payload_bytes: usize,
}

impl Default for SmfReadLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 64 * 1024 * 1024,
            max_header_bytes: 1024,
            max_tracks: 1024,
            max_track_bytes: 16 * 1024 * 1024,
            max_events: 1_000_000,
            max_event_payload_bytes: 1024 * 1024,
            max_total_payload_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ReadBudget {
    limits: SmfReadLimits,
    events: usize,
    payload_bytes: usize,
}

impl ReadBudget {
    pub(crate) fn new(limits: SmfReadLimits) -> Self {
        Self {
            limits,
            events: 0,
            payload_bytes: 0,
        }
    }

    pub(crate) fn claim_event(&mut self, offset: usize) -> Result<(), SmfError> {
        let actual = self.events.checked_add(1).ok_or(SmfError::LimitExceeded {
            offset,
            kind: SmfLimitKind::EventCount,
            actual: usize::MAX,
            maximum: self.limits.max_events,
        })?;
        enforce_limit(
            offset,
            SmfLimitKind::EventCount,
            actual,
            self.limits.max_events,
        )?;
        self.events = actual;
        Ok(())
    }

    pub(crate) fn claim_payload(&mut self, offset: usize, len: usize) -> Result<(), SmfError> {
        enforce_limit(
            offset,
            SmfLimitKind::EventPayloadBytes,
            len,
            self.limits.max_event_payload_bytes,
        )?;
        let total = self
            .payload_bytes
            .checked_add(len)
            .ok_or(SmfError::LimitExceeded {
                offset,
                kind: SmfLimitKind::TotalPayloadBytes,
                actual: usize::MAX,
                maximum: self.limits.max_total_payload_bytes,
            })?;
        enforce_limit(
            offset,
            SmfLimitKind::TotalPayloadBytes,
            total,
            self.limits.max_total_payload_bytes,
        )?;
        self.payload_bytes = total;
        Ok(())
    }
}

pub(crate) fn enforce_limit(
    offset: usize,
    kind: SmfLimitKind,
    actual: usize,
    maximum: usize,
) -> Result<(), SmfError> {
    if actual > maximum {
        return Err(SmfError::LimitExceeded {
            offset,
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
