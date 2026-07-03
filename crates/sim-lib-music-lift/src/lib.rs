//! Lifts music from lower-level representations to higher-level ones.
//!
//! A lift raises a concrete, low-level representation (a parsed MIDI/SMF file)
//! into a richer, more structured music representation: a `PianoRoll`, a
//! `DiffRoll` analysis view, a chord `Progression`, or a `Counterpoint` of
//! separated voices. Each lifter implements [`MidiLifter`] and returns a
//! [`LiftReport`] carrying the lifted value alongside diagnostics describing
//! lossy or ambiguous decisions.
//!
//! The `lift_to_*` free functions are convenience entry points over the
//! [`MidiToPianoRoll`], [`MidiToDiffRoll`], [`MidiToProgression`], and
//! [`MidiToCounterpoint`] lifters, and [`MusicLiftLib`] registers them as
//! host-side runtime exports.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod collect;
mod counterpoint;
mod model;
mod progression;
mod runtime;

pub use model::*;
pub use runtime::{MusicLiftLib, install_music_lift_lib};

#[cfg(test)]
mod tests;
