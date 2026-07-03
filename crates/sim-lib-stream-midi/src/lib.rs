#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! MIDI source, sink, and stream-spine adapters.
//!
//! This crate adapts the existing MIDI core memory/source/sink traits into
//! stream-core MIDI packets. It does not add host device backends.

mod codec;
mod packetize;
mod sink;
mod spine;
mod tracked;

pub use packetize::{
    midi_event_to_packet_event, midi_packet_to_lan_control_envelope, packetize_midi_source,
};
pub use sim_lib_stream_core::{MidiPacket, MidiPacketEvent};
pub use sink::{midi_packet_to_events, write_midi_packets_to_sink};
pub use spine::{midi_source_to_stream, midi_stream_to_sink};
pub use tracked::{TrackedMidiPacket, packetize_tracked_midi_source};

#[cfg(test)]
mod tests;
