//! Scales, modes, and scale-locking for the SIM music libraries.
//!
//! This crate defines the diatonic and symmetric [`Mode`]s, the [`Scale`] type
//! that anchors a mode to a tonic pitch class, and the diatonic operations built
//! on them (degree lookup, diatonic transposition, chord/scale tone mapping). The
//! [`PlayerScale`] and [`ScaleLockPlayer`] types provide a performance-oriented
//! surface that quantizes, filters, or remaps incoming pitches onto a chosen scale.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod player;

pub use model::*;
pub use player::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
