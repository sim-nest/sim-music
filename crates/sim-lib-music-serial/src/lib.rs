//! Immutable serial plans with stable identity, provenance, and partial order.
//!
//! This crate owns the score-adjacent structural source for serial practice:
//! validated row-instance identity, event identity, explicit role/origin
//! provenance, chord-safe simultaneous placement groups, and a precedence DAG
//! over immutable planned events. It does not realize MIDI, audio, notation,
//! transforms, or search.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod event;
mod evidence;
mod extract;
mod hypothesis;
mod order;
mod origin;
mod plan;
mod realization;
mod render;
mod strict;

pub use error::SerialPlanError;
pub use event::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId,
    SimultaneousGroupId, VoiceId,
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
pub use order::PrecedenceGraph;
pub use origin::{SerialOrigin, SerialRole};
pub use plan::SerialPlan;
pub use realization::{
    RealizedSerialEvent, RealizedSerialNote, RealizedSerialOrigin, SerialRealization,
    StrictRealizationError,
};
pub use render::{
    SerialRenderOptions, render_serial_piano_roll, render_serial_score, render_serial_staff,
};
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
mod tests_extract;
