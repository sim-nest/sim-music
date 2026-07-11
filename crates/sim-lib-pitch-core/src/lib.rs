//! Core pitch theory primitives for the SIM music libraries.
//!
//! This crate defines the foundational pitch types shared across the `sim-lib-pitch-*`
//! constellation: the mod-12 [`PitchClass`], the octave-aware [`Pitch`], the
//! letter-plus-accidental [`SpelledPitch`], and the [`Interval`] measured in
//! semitones. Higher-level crates (sets, scales, chords, namers) build on these
//! primitives rather than redefining their own pitch representations.
//!
//! Pitch classes use the twelve-tone equal-tempered convention where `C = 0` and
//! values increase by semitone up to `B = 11`. Octave-aware pitches use the MIDI
//! convention in which middle C (`C4`) is MIDI note 60.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
