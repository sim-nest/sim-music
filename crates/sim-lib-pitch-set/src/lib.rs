//! Pitch-class set representations and operations for the SIM music libraries.
//!
//! This crate models unordered collections of pitches as compact bitmasks. A
//! [`PitchClassMask`] packs the twelve pitch classes into a `u16`, supporting
//! rotation (transposition), inversion, prime-form normalization, and the
//! [`IntervalVector`] census used by set theory. [`PitchRangeMask`] does the same
//! across the full 128-key MIDI range. [`BitChord`] pairs a mask with an optional
//! root, and [`ThirdStackSignature`] encodes chords as stacks of minor and major
//! thirds.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

#[cfg(test)]
mod tests;
