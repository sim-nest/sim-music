//! Chords, voicings, and harmonic sequencing for the SIM music libraries.
//!
//! This crate builds chords from pitches, scale degrees, and jazz-style chord
//! symbols ([`Chord`], [`ChordSymbol`]), applies [`VoicingPolicy`] and
//! [`VelocityPolicy`] transformations, and drives generative players
//! ([`AutoChordPlayer`], [`ScalesChordsPlayer`]) that harmonize incoming pitches
//! against a scale. On top of these sit a wire-serializable chord progression
//! [`ChordSequencerPlayer`] and a roman-numeral-aware harmony suggester
//! ([`suggest_harmony`]).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;
mod player;
mod sequencer;
mod suggest;
mod voicing;

pub use model::*;
pub use player::*;
pub use sequencer::*;
pub use suggest::*;
pub use voicing::*;

#[cfg(test)]
mod tests;
