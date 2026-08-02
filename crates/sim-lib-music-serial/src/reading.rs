//! Named readings over immutable serial-plan evidence.

use crate::{PlannedSerialEvent, SerialRole};

/// Reproducible reading scope used when evaluating one serial practice.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SerialReading {
    /// Read only the structural row statement.
    StructuralPlan,
    /// Read all events while preserving their declared structural or derived roles.
    DeclaredRoles,
    /// Read every sounding event regardless of role boundaries.
    AllSounding,
}

impl SerialReading {
    /// Returns the stable reading token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StructuralPlan => "structural-plan",
            Self::DeclaredRoles => "declared-roles",
            Self::AllSounding => "all-sounding",
        }
    }

    pub(crate) const fn includes_event(self, event: &PlannedSerialEvent) -> bool {
        match self {
            Self::StructuralPlan => matches!(event.role, SerialRole::Structural),
            Self::DeclaredRoles | Self::AllSounding => true,
        }
    }
}
