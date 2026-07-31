//! Exact sounding windows and multi-domain consonance evaluation.
//!
//! This crate is the identity-preserving orchestration layer over SIM's exact
//! score conversion, MIDI realization, pitch sonance, psychoacoustic
//! dissonance, exact-ratio, commonality, and voice-leading owners. It splits
//! scores and realized MIDI at every exact onset and release, retains duplicate
//! notes and their source identities, and returns each metric independently.
//! There is deliberately no implicit scalar average.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod evaluate;
mod model;
mod runtime;
mod source;
mod windows;

pub use evaluate::{evaluate, evaluate_midi_timeline, evaluate_staff};
pub use model::*;
pub use runtime::{
    MusicConsonanceLib, install_music_consonance_lib, music_consonance_evaluate_symbol,
};
pub use windows::{slice_sounding_windows, sounding_windows};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
