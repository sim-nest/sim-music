//! Explicit serial allowances used to admit generic completion candidates.

use std::collections::BTreeSet;

use sim_lib_pitch_core::PitchClass;

use crate::OrdinalRef;

/// One caller-declared referential subset that may license non-structural additions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferentialSubsetAllowance {
    /// Stable subset identity retained in diagnostics and derived event provenance.
    pub id: String,
    /// Pitch classes admitted by the subset.
    pub pitch_classes: BTreeSet<PitchClass>,
}

impl ReferentialSubsetAllowance {
    /// Creates one named referential subset allowance.
    pub fn new(
        id: impl Into<String>,
        pitch_classes: impl IntoIterator<Item = PitchClass>,
    ) -> Result<Self, String> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err("referential subset id cannot be empty".to_owned());
        }
        let pitch_classes = pitch_classes.into_iter().collect::<BTreeSet<_>>();
        if pitch_classes.is_empty() {
            return Err("referential subset pitch classes cannot be empty".to_owned());
        }
        Ok(Self { id, pitch_classes })
    }
}

/// Serial material categories that may license one added note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialAllowanceKind {
    /// Reuse a pitch class already present in the current structural partition.
    CurrentPartition,
    /// Reuse a pitch class already stated earlier in the structural reading.
    StatedPitchClasses,
    /// Borrow a pitch class that remains in the structural aggregate ahead.
    AggregateRemainder,
    /// Borrow a caller-declared referential subset without claiming structure.
    ReferentialSubset {
        /// Stable subset identity.
        id: String,
    },
    /// Reuse a landed pitch class already exposed by an attached modal spine report.
    ModalProjection,
    /// Reuse non-structural derived material already present in the plan.
    DerivedReservoir,
    /// Reuse material already declared foreign by the plan.
    ExplicitForeignMaterial,
}

impl SerialAllowanceKind {
    pub(crate) fn label(&self) -> String {
        match self {
            Self::CurrentPartition => "current-partition".to_owned(),
            Self::StatedPitchClasses => "stated-pitch-classes".to_owned(),
            Self::AggregateRemainder => "aggregate-remainder".to_owned(),
            Self::ReferentialSubset { id } => format!("referential-subset/{id}"),
            Self::ModalProjection => "modal-projection".to_owned(),
            Self::DerivedReservoir => "derived-reservoir".to_owned(),
            Self::ExplicitForeignMaterial => "explicit-foreign-material".to_owned(),
        }
    }
}

/// One concrete allowance match for one added note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialAllowanceMatch {
    /// Category that licensed the note.
    pub kind: SerialAllowanceKind,
    /// Structural ordinals or cited sources associated with the match.
    pub ordinals: Vec<OrdinalRef>,
}

/// Admission policy layered over generic completion candidates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialCompletionAllowances {
    /// Admit notes that reuse the current structural partition.
    pub current_partition: bool,
    /// Admit notes that reuse already stated structural pitch classes.
    pub stated_pitch_classes: bool,
    /// Admit notes that borrow a future structural remainder pitch class.
    pub aggregate_remainder: bool,
    /// Admit notes covered by caller-declared referential subsets.
    pub referential_subsets: Vec<ReferentialSubsetAllowance>,
    /// Admit landed pitch classes already exposed by a modal spine report.
    pub modal_projection: bool,
    /// Admit pitch classes already present in non-structural derived material.
    pub derived_reservoir: bool,
    /// Admit pitch classes already present in explicit foreign material.
    pub explicitly_foreign_material: bool,
}

impl Default for SerialCompletionAllowances {
    fn default() -> Self {
        Self {
            current_partition: true,
            stated_pitch_classes: true,
            aggregate_remainder: false,
            referential_subsets: Vec::new(),
            modal_projection: false,
            derived_reservoir: false,
            explicitly_foreign_material: false,
        }
    }
}
