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

mod chroma;
mod constant_q;
mod frame;
mod model;
mod partial_track;
mod pipeline;
mod pitch_track;
mod runtime;

#[cfg(feature = "sound-music")]
mod music;

pub use chroma::*;
pub use constant_q::*;
pub use frame::*;
pub use model::*;
pub use partial_track::*;
pub use pitch_track::*;
pub use runtime::*;

#[cfg(feature = "sound-music")]
pub use music::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod partial_track_tests;
#[cfg(test)]
mod pitch_fixture_tests;
#[cfg(test)]
mod pitch_track_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transform_tests;
