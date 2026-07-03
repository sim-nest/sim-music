//! General MIDI sound set for the SIM music constellation.
//!
//! This crate provides the General MIDI drum map -- [`DrumSound`] and
//! [`DrumKeyMap`], including the standard kit and label resolution -- and
//! [`general_midi_bank`], which maps the 128 GM melodic programs onto concrete
//! timbres in a [`TimbreBank`](sim_lib_sound_bridge::TimbreBank).
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod model;

pub use model::*;

#[cfg(test)]
mod tests;
