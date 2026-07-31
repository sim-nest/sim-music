//! Audio-to-feature and audio-to-notes lifting for the SIM music constellation.
//!
//! This crate analyzes raw PCM audio and lifts it into pitched note
//! candidates. The [`AudioLifter`] trait and its [`FftPeakLifter`] and
//! [`HarmonicCombLifter`] implementations produce an [`AudioLiftResult`] of
//! per-window [`AudioLiftFrame`]s and assembled [`AudioNoteCandidate`]s, under
//! a configurable [`AudioLiftOptions`]. [`analyze_audio`] composes bounded onset,
//! beat, zero-crossing, perceptual/MFCC, chroma, key, and chord analysis while
//! retaining policy, confidence, alternatives, and delegated graph/HMM evidence.
//! With the `sound-music` feature, note results convert directly into music-core
//! piano rolls, diff rolls, and counterpoint.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod analysis;
mod beat;
mod chroma;
mod constant_q;
mod filterbank;
mod frame;
mod mfcc;
mod model;
mod onset;
mod partial_track;
mod pipeline;
mod pitch_track;
mod runtime;
mod runtime_analysis;
mod runtime_analysis_report;
mod runtime_pitch;
mod runtime_pitch_report;
mod zero_crossing;

#[cfg(feature = "sound-music")]
mod music;

pub use analysis::*;
pub use beat::*;
pub use chroma::*;
pub use constant_q::*;
pub use filterbank::*;
pub use frame::*;
pub use mfcc::*;
pub use model::*;
pub use onset::*;
pub use partial_track::*;
pub use pitch_track::*;
pub use runtime::*;
pub use runtime_analysis::*;
pub use runtime_pitch::*;
pub use zero_crossing::*;

#[cfg(feature = "sound-music")]
pub use music::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod analysis_tests;
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
