//! Audio-to-notes lifting for the SIM music constellation.
//!
//! This crate analyzes raw PCM audio and lifts it into pitched note
//! candidates. The [`AudioLifter`] trait and its [`FftPeakLifter`] and
//! [`HarmonicCombLifter`] implementations produce an [`AudioLiftResult`] of
//! per-window [`AudioLiftFrame`]s and assembled [`AudioNoteCandidate`]s, under
//! a configurable [`AudioLiftOptions`]. With the `sound-music` feature, results
//! convert directly into music-core piano rolls, diff rolls, and counterpoint.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod pipeline;
mod runtime;

#[cfg(feature = "sound-music")]
mod music;

pub use model::*;
pub use runtime::*;

#[cfg(feature = "sound-music")]
pub use music::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
