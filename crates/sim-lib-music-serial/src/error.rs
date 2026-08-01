//! Typed serial-plan validation failures.

use thiserror::Error;

use crate::{OrdinalRef, RowInstanceId, SerialEventId, SimultaneousGroupId, StructuralReadingId};

/// Failure while validating immutable serial-plan data.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SerialPlanError {
    /// A stable identifier was empty or used an unsafe character.
    #[error("invalid {kind} id {value:?}: {reason}")]
    InvalidId {
        /// Identity kind being validated.
        kind: &'static str,
        /// Rejected text.
        value: String,
        /// Specific syntax failure.
        reason: &'static str,
    },
    /// A planned event referenced a missing row instance.
    #[error("event {event_id} references unknown row {row_id}")]
    UnknownRow {
        /// Planned event carrying the bad reference.
        event_id: SerialEventId,
        /// Referenced row id.
        row_id: RowInstanceId,
    },
    /// A planned event referenced an ordinal outside the source row.
    #[error(
        "event {event_id} references ordinal {ordinal} outside row {row_id} with length {row_len}"
    )]
    OrdinalOutOfRange {
        /// Planned event carrying the bad reference.
        event_id: SerialEventId,
        /// Referenced row instance.
        row_id: RowInstanceId,
        /// Rejected ordinal.
        ordinal: usize,
        /// Row length expected by the referenced row form.
        row_len: usize,
    },
    /// The same structural role rules were violated.
    #[error("event {event_id} has role/origin mismatch: {reason}")]
    RoleOriginMismatch {
        /// Affected event.
        event_id: SerialEventId,
        /// Human-readable validation reason.
        reason: &'static str,
    },
    /// A non-structural event omitted explicit parent evidence.
    #[error("event {event_id} requires at least one parent for role {role}")]
    MissingParents {
        /// Affected event.
        event_id: SerialEventId,
        /// Role name.
        role: &'static str,
    },
    /// A parent event id did not resolve.
    #[error("event {event_id} names unknown parent {parent_id}")]
    UnknownParent {
        /// Child event.
        event_id: SerialEventId,
        /// Missing parent id.
        parent_id: SerialEventId,
    },
    /// A parent relation named the event itself.
    #[error("event {0} cannot name itself as a parent")]
    SelfParent(SerialEventId),
    /// Parent evidence formed a cycle.
    #[error("parent evidence contains a cycle involving event {0}")]
    ParentCycle(SerialEventId),
    /// Structural coverage failed to cover every ordinal of one row.
    #[error("row {row_id} is missing structural coverage for ordinals {ordinals:?}")]
    MissingStructuralCoverage {
        /// Row instance lacking complete structural source coverage.
        row_id: RowInstanceId,
        /// Zero-based ordinals never cited by a structural event.
        ordinals: Vec<usize>,
    },
    /// The precedence graph named an unknown event.
    #[error("precedence graph names unknown event {0}")]
    UnknownPrecedenceNode(SerialEventId),
    /// The precedence graph contained a self-loop.
    #[error("precedence graph contains a self-edge on event {0}")]
    SelfPrecedence(SerialEventId),
    /// The precedence graph contained a directed cycle.
    #[error("precedence graph contains a cycle involving event {0}")]
    PrecedenceCycle(SerialEventId),
    /// Precedence claimed an ordering between events declared simultaneous.
    #[error("precedence graph orders simultaneous-group {group_id} members {before} and {after}")]
    SimultaneousPrecedenceConflict {
        /// Simultaneous group shared by both events.
        group_id: SimultaneousGroupId,
        /// Event required to occur before the other.
        before: SerialEventId,
        /// Event required to occur after the other.
        after: SerialEventId,
    },
    /// An event carried no ordinal references.
    #[error("event {0} must reference at least one structural ordinal")]
    EmptyOrdinalSet(SerialEventId),
    /// The same ordinal was repeated within one event.
    #[error("event {event_id} repeats ordinal reference {ordinal:?}")]
    DuplicateOrdinal {
        /// Affected event.
        event_id: SerialEventId,
        /// Repeated ordinal reference.
        ordinal: OrdinalRef,
    },
    /// An event omitted the structural licenses required for reporting.
    #[error("event {0} must declare at least one structural license")]
    MissingStructuralLicenses(SerialEventId),
    /// A structural license explanation was empty.
    #[error("structural reading {0} must provide a non-empty rationale")]
    EmptyStructuralLicenseRationale(StructuralReadingId),
}
