#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Analysis of musical material for the SIM music libraries.
//!
//! This crate derives structural views over music objects. [`DiffRoll`] turns a
//! `sim_lib_music_core::PianoRoll` into per-event frames of sounding, starting,
//! ending, and slurred pitches, and [`ChordWindow`] segments that timeline into
//! chord-bearing intervals with pitch-range, pitch-class, and bit-chord masks.
//! With the `discrete-fwht` feature, the `walsh` module adds Walsh-Hadamard
//! spectral analysis of melodies, contours, and pitch-class windows.

mod model;

pub use model::*;

#[cfg(feature = "discrete-fwht")]
pub mod walsh;

#[cfg(test)]
mod tests;
