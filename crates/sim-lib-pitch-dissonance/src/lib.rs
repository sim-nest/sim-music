//! Dissonance and harmonic-complexity scoring for the SIM music libraries.
//!
//! This crate scores pitch-class sets against a registry of pluggable dissonance
//! [`PitchDissonanceModel`]s: an interval-vector weighting, a Forte-style
//! complexity measure, a key-relative tonal-function model, and a tritone-density
//! ratio. The [`PitchDissonanceRegistry`] runs every model at once, and the
//! [`PitchDissonanceLib`] exposes the models as a SIM runtime library installable
//! through [`install_pitch_dissonance_lib`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod runtime;

pub use model::*;
pub use runtime::{PitchDissonanceLib, install_pitch_dissonance_lib};

#[cfg(test)]
mod tests;
