//! Chords, voicings, and harmonic sequencing for the SIM music libraries.
//!
//! This crate builds chords from pitches, scale degrees, and jazz-style chord
//! symbols ([`Chord`], [`ChordSymbol`]), applies [`VoicingPolicy`] and
//! [`VelocityPolicy`] transformations, and drives generative players
//! ([`AutoChordPlayer`], [`ScalesChordsPlayer`]) that harmonize incoming pitches
//! against a scale. [`HarmonyProgram`] keeps chord palettes, cadence chains,
//! palette algebra, hard constraints, weighted metrics, voicing changes, and
//! export settings as codec-neutral data. Hard-rule and soft-score evidence
//! remain separate through [`evaluate_harmony`]. [`plan_harmony`] applies the
//! same declarative problem through exhaustive, factored, certified layered,
//! or bounded beam planning while retaining failed-rule and search receipts.
//! On top of these sit a
//! wire-serializable chord progression
//! [`ChordSequencerPlayer`] and a roman-numeral-aware harmony suggester
//! ([`suggest_harmony`]).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod harmonize;
mod harmonize_layered;
mod harmonize_model;
mod harmony_eval;
mod harmony_expr;
mod harmony_expr_support;
mod harmony_metric;
mod harmony_model;
mod harmony_palette;
mod harmony_rule_expr;
mod harmony_rules;
mod model;
mod player;
mod sequencer;
mod suggest;
mod voicing;
mod voicing_change;

pub use harmonize::*;
pub use harmonize_model::*;
pub use harmony_eval::*;
pub use harmony_metric::*;
pub use harmony_model::*;
pub use harmony_palette::*;
pub use harmony_rules::*;
pub use model::*;
pub use player::*;
pub use sequencer::*;
pub use suggest::*;
pub use voicing::*;
pub use voicing_change::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod harmonize_tests;
#[cfg(test)]
mod harmony_conformance;
#[cfg(test)]
mod harmony_tests;
#[cfg(test)]
mod tests;
