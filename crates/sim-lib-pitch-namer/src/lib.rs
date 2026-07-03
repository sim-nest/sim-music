//! Multi-school cluster naming for the SIM music libraries.
//!
//! This crate is the aggregator over the pitch-naming schools. It defines the
//! shared [`ClusterNamer`] trait and the [`NamingSchool`] taxonomy, then drives
//! every built-in school -- Forte set-class names, functional roman numerals,
//! plain set-theory prime forms, neo-Riemannian labels, and jazz chord symbols --
//! through one [`NamerRegistry`]. The registry can label a pitch-class set in
//! every school at once and translate a label from one school to another. The
//! schools themselves live in the `sim-lib-pitch-namer-*` sibling crates; this
//! crate composes them and exposes them as a SIM runtime library through
//! [`PitchNamerLib`] / [`install_pitch_namer_lib`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod registry;
mod runtime;
mod set_theory;
mod types;

pub use registry::*;
pub use runtime::{PitchNamerLib, install_pitch_namer_lib};
pub use types::*;

#[cfg(test)]
mod tests;
