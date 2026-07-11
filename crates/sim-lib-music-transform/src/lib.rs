//! Music transformation layer for the SIM music constellation.
//!
//! This crate applies transformations to musical material: classic operations
//! such as transpose, invert, retrograde, augment, and diminish; configurable
//! pitch and time remaps; pattern mutators; and a capability-gated custom event
//! filter pipeline. Transforms read a `MusicObject` into a canonical
//! `PianoRoll` and return new `Music`, optionally paired with diagnostics in a
//! [`TransformReport`].
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arranger;
mod diagnostic;
mod filter;
mod filter_eval;
mod model;
mod mutator;
mod player;
mod remap;

pub use arranger::*;
pub use diagnostic::*;
pub use filter::*;
pub use model::*;
pub use mutator::*;
pub use player::*;
pub use remap::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
