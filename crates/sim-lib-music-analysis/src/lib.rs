#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Analysis of musical material for the SIM music libraries.
//!
//! This crate derives structural views over music objects. [`DiffRoll`] turns a
//! `sim_lib_music_core::PianoRoll` into per-event frames of sounding, starting,
//! ending, and slurred pitches, and [`ChordWindow`] segments that timeline into
//! chord-bearing intervals with pitch-range, pitch-class, and bit-chord masks.
//! [`decode_keys`] and [`decode_chords`] adapt chroma/features and declared
//! templates to shared finite-HMM inference while retaining posterior evidence.
//! With the `discrete-fwht` feature, the `walsh` module adds Walsh-Hadamard
//! spectral analysis of melodies, contours, and pitch-class windows.

mod harmonic;
mod harmonic_templates;
mod model;

pub use harmonic::*;
pub use harmonic_templates::*;
pub use model::*;

#[cfg(feature = "discrete-fwht")]
pub mod walsh;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod harmonic_tests;
#[cfg(test)]
mod tests;
