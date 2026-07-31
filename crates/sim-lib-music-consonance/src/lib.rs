//! Exact consonance evaluation and reversible additive completion.
//!
//! This crate is the identity-preserving orchestration layer over SIM's exact
//! score conversion, MIDI realization, pitch sonance, psychoacoustic
//! dissonance, exact-ratio, commonality, and voice-leading owners. It splits
//! scores and realized MIDI at every exact onset and release, retains duplicate
//! notes and their source identities, and returns each metric independently. It
//! can also search typed note, ornament, chord, pedal, doubling, and voice
//! additions under explicit metric and style constraints by reusing the
//! generic bounded discrete search engine. Completion patches are content-bound
//! and exactly removable; there is deliberately no implicit scalar average or
//! destructive rewrite.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod completion;
mod constraints;
mod evaluate;
mod model;
mod patch;
mod runtime;
mod source;
mod windows;

pub use completion::*;
pub use constraints::*;
pub use evaluate::{evaluate, evaluate_midi_timeline, evaluate_staff};
pub use model::*;
pub use patch::*;
pub use runtime::{
    MusicConsonanceLib, install_music_consonance_lib, music_consonance_evaluate_symbol,
};
pub use windows::{slice_sounding_windows, sounding_windows};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod tests;
