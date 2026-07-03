//! Real-time MIDI buffering for the SIM music stack.
//!
//! This crate provides fixed-capacity ring buffers that bridge real-time MIDI
//! I/O into the [`MidiSource`](sim_lib_midi_core::MidiSource)/
//! [`MidiSink`](sim_lib_midi_core::MidiSink) traits: [`RingMidiBuffer`] acts as
//! both source and sink, and [`RingTrackedMidiBuffer`] adds per-track tagging.
//! When a buffer is full the oldest event is dropped and counted, so a slow
//! consumer never blocks a real-time producer. The host-registered
//! [`MidiLiveLib`] publishes these buffers as runtime plugin rows.
//!
//! # Examples
//!
//! A ring buffer accepts written events and yields them back in order:
//!
//! ```
//! use sim_lib_midi_live::RingMidiBuffer;
//! use sim_lib_midi_core::{
//!     MetaEvent, MidiEvent, MidiPayload, MidiSink, MidiSource, TickTime,
//!     synthetic_origin,
//! };
//!
//! let mut buffer = RingMidiBuffer::new(480, 4).unwrap();
//! let event = MidiEvent {
//!     time: TickTime::new(0, 480).unwrap(),
//!     origin: synthetic_origin(),
//!     payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
//! };
//! buffer.write(&event).unwrap();
//! assert_eq!(buffer.len(), 1);
//! assert_eq!(buffer.next().unwrap(), Some(event));
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod ring;
mod runtime;
mod session;

pub use error::*;
pub use ring::*;
pub use runtime::*;
pub use session::*;

#[cfg(test)]
mod tests;
