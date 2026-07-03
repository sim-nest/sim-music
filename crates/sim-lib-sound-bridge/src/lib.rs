//! MIDI-to-sound bridging for the SIM music constellation.
//!
//! This crate connects the MIDI surface to the sound layer. The
//! [`MidiToSoundBridge`] consumes MIDI events and produces [`ScheduledTone`]s,
//! resolving programs through a [`TimbreBank`], pitches through a tuning, and
//! polyphony through a [`VoicePool`]. [`BridgeOptions`] and
//! [`BridgeChannelState`] configure and track per-channel behavior, and a
//! runtime surface installs the bridge cards as a SIM lib.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bank;
mod bridge;
mod error;
mod pool;
mod runtime;

pub use bank::*;
pub use bridge::*;
pub use error::*;
pub use pool::*;
pub use runtime::*;

#[cfg(test)]
mod tests;
