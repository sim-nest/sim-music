//! Sound rendering for the SIM music constellation.
//!
//! This crate turns synthesized [`Tone`](sim_lib_sound_core::Tone)s into
//! interleaved PCM audio. [`PcmRenderer`] renders single tones and mixes
//! scheduled tones with per-tone timing and panning, and encodes the result as
//! 16-bit WAV; [`RendererOptions`] configures the sample rate and channel
//! count.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod model;
mod runtime;

pub use error::*;
pub use model::*;
pub use runtime::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
