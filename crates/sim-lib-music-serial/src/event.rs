//! Stable ids and immutable planned serial events.

use std::fmt::{Display, Formatter};

use sim_lib_music_core::ObjectId;

use crate::{SerialOrigin, SerialPlanError, SerialRole};

fn validate_id_text(
    kind: &'static str,
    value: impl Into<String>,
) -> Result<String, SerialPlanError> {
    let value = value.into();
    if value.trim().is_empty() {
        return Err(SerialPlanError::InvalidId {
            kind,
            value,
            reason: "value cannot be empty",
        });
    }
    if value
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.')))
    {
        return Err(SerialPlanError::InvalidId {
            kind,
            value,
            reason: "value must use ASCII letters, digits, /, -, _, or .",
        });
    }
    Ok(value)
}

macro_rules! stable_id {
    ($name:ident, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Creates a validated stable identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, SerialPlanError> {
                Ok(Self(validate_id_text($kind, value)?))
            }

            /// Returns the stable wire text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

stable_id!(
    RowInstanceId,
    "row-instance",
    "Stable identity for one row instance in a serial plan."
);
stable_id!(
    SerialEventId,
    "serial-event",
    "Stable identity for one planned serial event."
);
stable_id!(
    SimultaneousGroupId,
    "simultaneous-group",
    "Stable identity for one equal-onset simultaneous event group."
);

/// Stable voice identity reused from the exact music-core score model.
pub type VoiceId = ObjectId;

/// One stable structural ordinal within a specific row instance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrdinalRef {
    /// Row instance owning the ordinal.
    pub row_id: RowInstanceId,
    /// Zero-based ordinal within that row instance.
    pub ordinal: usize,
}

impl OrdinalRef {
    /// Creates one stable row/ordinal reference.
    pub fn new(row_id: RowInstanceId, ordinal: usize) -> Self {
        Self { row_id, ordinal }
    }
}

/// Placement metadata that preserves simultaneity without inventing a chord order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventPlacement {
    simultaneous_group: Option<SimultaneousGroupId>,
}

impl EventPlacement {
    /// Returns an event placement with no simultaneous chord/group membership.
    pub const fn independent() -> Self {
        Self {
            simultaneous_group: None,
        }
    }

    /// Returns an event placement belonging to one simultaneous group.
    pub fn simultaneous(group: SimultaneousGroupId) -> Self {
        Self {
            simultaneous_group: Some(group),
        }
    }

    /// Returns the optional simultaneous group id.
    pub fn simultaneous_group(&self) -> Option<&SimultaneousGroupId> {
        self.simultaneous_group.as_ref()
    }
}

/// One immutable planned serial event with row provenance, voice identity, and parent evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedSerialEvent {
    /// Stable event identity.
    pub id: SerialEventId,
    /// Structural row ordinals this event realizes together.
    pub ordinals: Vec<OrdinalRef>,
    /// Structural, derived, ornamental, or external role.
    pub role: SerialRole,
    /// Role-specific origin/provenance details.
    pub origin: SerialOrigin,
    /// Stable voice identity for the event.
    pub voice: VoiceId,
    /// Simultaneous placement metadata.
    pub placement: EventPlacement,
    /// Explicit parent evidence.
    pub parents: Vec<SerialEventId>,
}
