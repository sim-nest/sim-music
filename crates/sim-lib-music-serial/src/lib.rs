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
mod array;
mod canon;
mod chromatic;
mod cycle;
mod deploy;
mod derived;
mod error;
mod event;
mod evidence;
mod extract;
mod hypothesis;
mod integral;
mod invariant;
mod modal;
mod nesting;
mod order;
mod origin;
mod parameter;
mod plan;
mod practice;
mod practice_builtin;
mod reading;
mod realization;
mod realizer;
mod referential;
mod registry;
mod render;
mod report;
mod spine;
mod strict;
mod techniques;
mod time_point;

pub use anchor::ReferentialEmphasis;
pub use array::{
    AggregateArrayReport, AggregatePartitionReport, ColumnPartition, PartitionCoverageReport,
    SerialArray, SerialArrayError, SerialArrayRow, VerticalAggregateRequirement,
};
pub use canon::{
    CanonDeployment, CanonError, CanonOrchestration, CanonRealizationEvent, CanonSpec,
    CanonSymmetryCertificate, CanonSymmetryRequirement, CanonVoiceProfile, CanonVoiceSpec,
    build_canon,
};
pub use chromatic::{ChromaticSerialRealizer, strict_chromatic_realizer_id};
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
pub use integral::{
    ArticulationTrack, BoundParameterTrack, DurationTrack, DynamicsTrack, ErasedParameterBinding,
    Exhaustion, IntegralError, IntegralPlan, ParameterOrdinalLedgerEntry, ParameterProjection,
    ParameterStep, ParameterTrack, RegisterTrack, TimbreTrack,
};
pub use invariant::{EvidenceId, InvariantLedger, InvariantLedgerEntry, InvariantStatus, WaiverId};
pub use modal::{
    MarkedChromaticInflectionRealizer, ModalDegreeCycleRealizer, NearestScaleToneRealizer,
    NonPitchSpineRealizer,
};
pub use nesting::{
    NestedSerialValue, NestingError, NestingExpansion, NestingLimits, expand_nested,
    rotate_sequence_left,
};
pub use order::PrecedenceGraph;
pub use origin::{SerialOrigin, SerialRole};
pub use parameter::{ParameterAlphabet, ParameterError, ParameterSeries, ParameterValue};
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
pub use realizer::{
    EventSound, RealizationContext, RealizationService, RealizationServices, RealizerId,
    RegisterBounds, SerialRealizer, SimultaneousRenderPolicy, StrictEventSpec, StrictPitchLayout,
    StrictRealizationContext, TiePolicy, VoiceBounds,
};
pub use referential::{
    ReferentialClaim, ReferentialContextReport, ReferentialEvidence, ReferentialEvidenceKind,
    ReferentialRatioSummary, ReferentialReport, ReferentialRequest, analyze_referential_subset,
};
pub use registry::{SerialRealizerRegistry, default_realizer_registry};
pub use render::{
    SerialRenderOptions, render_serial_piano_roll, render_serial_score, render_serial_staff,
};
pub use report::SerialPracticeReport;
pub use spine::{
    ChromaticAggregateIdentity, SerialRepeatedDegree, SerialSonanceContext, SerialSpineCollision,
    SerialSpineEntry, SerialSpineKind, SerialSpineLabel, SerialSpineReport,
};
pub use strict::realize_strict;
pub use time_point::{TimePointAlphabet, TimePointError, TimePointRow, TimePointSystem};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_array_time_point;
#[cfg(test)]
mod tests_deploy;
#[cfg(test)]
mod tests_extract;
#[cfg(test)]
mod tests_integral;
#[cfg(test)]
mod tests_realizer;
#[cfg(test)]
mod tests_techniques;
