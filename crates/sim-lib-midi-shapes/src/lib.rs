//! Shape and citizen surfaces that expose the MIDI model to the SIM runtime.
//!
//! This crate bridges the [`sim_lib_midi_core`]/[`sim_lib_midi_smf`] data types
//! into two SIM facilities:
//!
//! - A string codec (the `encode_*`/`decode_*` functions) that round-trips
//!   MIDI events, channel messages, meta events, SysEx, raw bytes, tracks, and
//!   SMF files through a compact `#(...)` reader form, with errors reported as
//!   [`MidiShapeError`].
//! - Citizen descriptors ([`MidiEventDescriptor`], [`MidiChannelMessageDescriptor`],
//!   [`MidiMetaEventDescriptor`], [`MidiSmfTrackDescriptor`],
//!   [`MidiSmfFileDescriptor`]) that wrap a canonical form as a runtime citizen,
//!   plus the host-registered [`MidiShapesLib`] that publishes the `midi/*`
//!   shape values.
//!
//! # Examples
//!
//! The channel-message codec round-trips through the reader form:
//!
//! ```
//! use sim_lib_midi_shapes::{decode_channel_message, encode_channel_message};
//!
//! let form = "#(Channel NoteOn 0 60 100)";
//! let message = decode_channel_message(form).unwrap();
//! assert_eq!(encode_channel_message(message), form);
//! ```

#![forbid(unsafe_code)]
#![allow(deprecated)]
#![deny(missing_docs)]

mod citizen;
mod codec;
mod runtime;

pub use citizen::{
    MidiChannelMessageDescriptor, MidiEventDescriptor, MidiMetaEventDescriptor,
    MidiSmfFileDescriptor, MidiSmfTrackDescriptor, midi_channel_message_class_symbol,
    midi_event_class_symbol, midi_meta_event_class_symbol, midi_smf_file_class_symbol,
    midi_smf_track_class_symbol,
};
pub use codec::*;
pub use runtime::{MidiShapesLib, install_midi_shapes_lib};

#[cfg(test)]
mod tests;
