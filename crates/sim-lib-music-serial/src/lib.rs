//! Immutable serial plans with stable identity, provenance, and partial order.
//!
//! This crate owns the score-adjacent structural source for serial practice:
//! validated row-instance identity, event identity, explicit role/origin
//! provenance, chord-safe simultaneous placement groups, and a precedence DAG
//! over immutable planned events. It does not realize MIDI, audio, notation,
//! transforms, or search.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod anchor;
mod canon;
mod cycle;
mod deploy;
mod derived;
mod error;
mod event;
mod evidence;
mod extract;
mod hypothesis;
mod invariant;
mod order;
mod origin;
mod plan;
mod practice;
mod practice_builtin;
mod reading;
mod realization;
mod referential;
mod render;
mod report;
mod strict;
mod techniques;

pub use anchor::ReferentialEmphasis;
pub use canon::{
    CanonDeployment, CanonError, CanonOrchestration, CanonRealizationEvent, CanonSpec,
    CanonSymmetryCertificate, CanonSymmetryRequirement, CanonVoiceProfile, CanonVoiceSpec,
    build_canon,
};
pub use cycle::{
    CyclicOrder, CyclicProjection, CyclicProjectionSpec, ParameterTrackKind, project_cyclic_order,
};
pub use deploy::{
    AggregateRotationSpec, InterlockingPartitionSpec, MelodyAccompanimentSpec, SerialDeployError,
    SerialDeployer, SerialDeployerKind, SerialDeployerParameter, SerialDeployerSpec,
    SimultaneousFormsSpec, TechniquePlan, TechniquePlanBuilder, VerticalBlocksSpec,
    complete_horizontal_statement, interlocking_partition, melody_accompaniment_distribution,
    motivic_partition, schoenberg_partitioned, simultaneous_forms, strict_aggregate,
    verticalize_selected_blocks,
};
pub use derived::{
    DerivedCellDeployment, DerivedCellOccurrence, InvariantCertificate, InvariantFormCandidate,
    InvariantFormCandidates, InvariantRequirement, SymmetryCertificate, SymmetryRequirement,
    UnsatisfiedInvariantRequest, deploy_derived_cells, forms_with_invariant,
};
pub use error::SerialPlanError;
pub use event::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId,
    SimultaneousGroupId, StructuralLicense, StructuralReadingId, VoiceId,
};
pub use evidence::{ExtractionEvidence, ExtractionOutcome};
pub use extract::{
    SerialExtractionError, SerialExtractionRequest, SerialExtractionServices,
    extract_serial_hypotheses,
};
pub use hypothesis::{
    RankedSerialHypothesis, SerialAliasEvidence, SerialObservation, SerialObservationBlock,
    SerialReadingOrder, SerialStableRank, SerialTimeSpan,
};
pub use invariant::{EvidenceId, InvariantLedger, InvariantLedgerEntry, InvariantStatus, WaiverId};
pub use order::PrecedenceGraph;
pub use origin::{SerialOrigin, SerialRole};
pub use plan::SerialPlan;
pub use practice::{
    BuiltInPracticeRule, DeclaredWaivers, PracticeId, PracticeRule, PracticeRuleId,
    PracticeRuleKind, PracticeRuleParameter, PracticeRuleSpec, SerialPractice,
};
pub use reading::SerialReading;
pub use realization::{
    RealizedSerialEvent, RealizedSerialNote, RealizedSerialOrigin, SerialRealization,
    StrictRealizationError,
};
pub use referential::{
    ReferentialClaim, ReferentialContextReport, ReferentialEvidence, ReferentialEvidenceKind,
    ReferentialRatioSummary, ReferentialReport, ReferentialRequest, analyze_referential_subset,
};
pub use render::{
    SerialRenderOptions, render_serial_piano_roll, render_serial_score, render_serial_staff,
};
pub use report::SerialPracticeReport;
pub use strict::{
    EventSound, SimultaneousRenderPolicy, StrictEventSpec, StrictPitchLayout,
    StrictRealizationContext, TiePolicy, realize_strict,
};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_deploy;
#[cfg(test)]
mod tests_extract;
#[cfg(test)]
mod tests_techniques;
