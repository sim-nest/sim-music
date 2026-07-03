//! Canonical MIDI channel-message wire bytes.
//!
//! The status-nibble + data-byte layout of a [`ChannelMessage`] is fixed by the
//! MIDI spec; the SMF writer and the wasm frame model each hand-rolled the same
//! `match`. This is the one home for it (OVERLAP6.14).

use crate::ChannelMessage;

/// Encode a channel message to its `(status, data)` wire bytes.
///
/// Pitch-bend's two data bytes are masked to 7 bits each, per the MIDI spec.
pub fn encode_channel(message: &ChannelMessage) -> (u8, Vec<u8>) {
    match *message {
        ChannelMessage::NoteOff { ch, key, vel } => (0x80 | ch.0, vec![key.0, vel.0]),
        ChannelMessage::NoteOn { ch, key, vel } => (0x90 | ch.0, vec![key.0, vel.0]),
        ChannelMessage::PolyAftertouch { ch, key, pressure } => {
            (0xa0 | ch.0, vec![key.0, pressure.0])
        }
        ChannelMessage::ControlChange { ch, cc, value } => (0xb0 | ch.0, vec![cc.0, value.0]),
        ChannelMessage::ProgramChange { ch, program } => (0xc0 | ch.0, vec![program.0]),
        ChannelMessage::ChanAftertouch { ch, pressure } => (0xd0 | ch.0, vec![pressure.0]),
        ChannelMessage::PitchBend { ch, value } => (
            0xe0 | ch.0,
            vec![(value.0 & 0x7f) as u8, ((value.0 >> 7) & 0x7f) as u8],
        ),
    }
}
