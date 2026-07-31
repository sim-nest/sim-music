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
//! [`quantize_staff`] globally aligns exact onsets to declared tempo/meter,
//! swing, and tuplet lattices. [`compare_sequences`] states its melody/rhythm
//! feature and transposition/time-scale invariances while retaining shared DTW
//! and correlation evidence. [`discover_patterns`] admits bounded hash
//! candidates, exact-verifies occurrences through shared discrete search, and
//! retains identities, affine transforms, overlap policy, and receipts.
//! [`MusicAlgorithmPlanLib`] adds the open `music/algorithm-plan` application
//! seam: independently loaded stage functions register under a data stage name,
//! and Shape ranking selects the implementation for each stage request.
//! The [`tonnetz`] module adds canonical-triad P/L/R actions and certified,
//! deterministic shortest paths while keeping chord identity independent of
//! Riemannian display labels.
//! With the `discrete-fwht` feature, the `walsh` module adds Walsh-Hadamard
//! spectral analysis of melodies, contours, and pitch-class windows.

mod event;
mod foundry;
mod harmonic;
mod harmonic_templates;
mod model;
mod pattern;
mod quantize;
mod similarity;
pub mod tonnetz;

pub use event::*;
pub use foundry::*;
pub use harmonic::*;
pub use harmonic_templates::*;
pub use model::*;
pub use pattern::*;
pub use quantize::*;
pub use similarity::*;

#[cfg(feature = "discrete-fwht")]
pub mod walsh;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod harmonic_tests;
#[cfg(test)]
mod sequence_tests;
#[cfg(test)]
mod tests;
