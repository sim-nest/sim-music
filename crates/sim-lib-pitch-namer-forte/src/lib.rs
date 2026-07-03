//! Forte set-class naming for the SIM music libraries.
//!
//! This crate implements the Forte naming school: it maps a pitch-class set to its
//! Forte set-class name (such as `4-27` for a dominant seventh) via a lookup
//! [`FORTE_TABLE`] of prime-form masks. [`lookup_forte_label`] normalizes a
//! [`PitchClassMask`](sim_lib_pitch_set::PitchClassMask) to prime form before
//! matching, so any transposition or rotation of a set resolves to the same name.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod table;

pub use table::*;

#[cfg(test)]
mod tests;
