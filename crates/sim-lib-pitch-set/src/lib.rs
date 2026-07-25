//! Pitch-class set representations and operations for the SIM music libraries.
//!
//! This crate models unordered collections of pitches as compact bitmasks. A
//! [`PitchClassMask`] packs the twelve pitch classes into a `u16`, supporting
//! rotation (transposition), inversion, numeric normalization, conventional
//! normal-order and prime-form classification, complements, set inclusion,
//! symmetry, Z-relation, interval/gap forms, graph neighborhoods, and the
//! [`IntervalVector`] census used by set theory. [`PitchRangeMask`] does the
//! same across the full 128-key MIDI range. [`BitChord`] pairs a mask with an
//! optional root, and [`ThirdStackSignature`] encodes chords as stacks of minor
//! and major thirds.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod conventional;
mod geometry;
mod model;

pub use conventional::*;
pub use geometry::*;
pub use model::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
