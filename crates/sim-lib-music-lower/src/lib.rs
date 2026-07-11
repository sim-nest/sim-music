//! Lowers music from higher-level representations to renderable ones.
//!
//! Lowering is the inverse of lifting: it takes a structured music object or
//! [`Score`](sim_lib_music_core::Score) and renders it down to a concrete,
//! playable Standard MIDI File. [`lower`] and [`lower_score`] produce an
//! in-memory `SmfFile`, while [`write_smf`] serializes that file to bytes.
//! [`LowerOpts`] controls ticks-per-quarter resolution, the tempo map, and how
//! voices are split across MIDI tracks.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod piano_roll;

pub use model::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
