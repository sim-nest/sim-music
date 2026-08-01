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
mod order;
mod origin;
mod plan;

pub use error::SerialPlanError;
pub use event::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId,
    SimultaneousGroupId, VoiceId,
};
pub use order::PrecedenceGraph;
pub use origin::{SerialOrigin, SerialRole};
pub use plan::SerialPlan;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
