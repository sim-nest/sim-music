//! Dissonance and harmonic-complexity scoring for the SIM music libraries.
//!
//! This crate scores pitch-class sets against a registry of pluggable dissonance
//! [`PitchDissonanceModel`]s: an interval-vector weighting, a Forte-style
//! complexity measure, a key-relative tonal-function model, and a tritone-density
//! ratio. It also compares contextual pitch windows with multiplicity-aware
//! roughness, commonality, leading, motion, pseudo-partial, interval-vector, and
//! exact-ratio components through [`ContextualSonanceRegistry`]. The
//! [`PitchDissonanceRegistry`] runs every pitch-class model at once, and the
//! [`PitchDissonanceLib`] exposes the models as a SIM runtime library installable
//! through [`install_pitch_dissonance_lib`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod contextual;
mod contextual_support;
mod model;
mod runtime;

pub use contextual::*;
pub use model::*;
pub use runtime::{PitchDissonanceLib, install_pitch_dissonance_lib};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
