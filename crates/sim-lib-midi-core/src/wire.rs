//! Canonical MIDI channel-message wire bytes.
//!
//! The status-nibble + data-byte layout of a [`ChannelMessage`] is fixed by the
//! MIDI spec; the SMF writer and the wasm frame model each hand-rolled the same
//! `match`. This is the one home for it (OVERLAP6.14).

use crate::{Channel, ChannelMessage, MidiError, U7, U14};

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

/// Decode a channel message from its `(status, data)` wire bytes: the inverse
/// of [`encode_channel`].
///
/// The channel is taken from the low nibble of `status` and the message type
/// from the high nibble. Each consumed data byte must be a valid 7-bit value
/// ([`MidiError::InvalidU7`]); a status whose high nibble is not a channel-voice
/// type yields [`MidiError::NotChannelStatus`], and a data slice shorter than
/// the message requires yields [`MidiError::TruncatedChannel`]. Pitch-bend's
/// two 7-bit data bytes are recombined into the 14-bit value.
pub fn decode_channel(status: u8, data: &[u8]) -> Result<ChannelMessage, MidiError> {
    let ch = Channel::new(status & 0x0f)?;
    let message = match status & 0xf0 {
        0x80 => ChannelMessage::NoteOff {
            ch,
            key: data_u7(data, 0)?,
            vel: data_u7(data, 1)?,
        },
        0x90 => ChannelMessage::NoteOn {
            ch,
            key: data_u7(data, 0)?,
            vel: data_u7(data, 1)?,
        },
        0xa0 => ChannelMessage::PolyAftertouch {
            ch,
            key: data_u7(data, 0)?,
            pressure: data_u7(data, 1)?,
        },
        0xb0 => ChannelMessage::ControlChange {
            ch,
            cc: data_u7(data, 0)?,
            value: data_u7(data, 1)?,
        },
        0xc0 => ChannelMessage::ProgramChange {
            ch,
            program: data_u7(data, 0)?,
        },
        0xd0 => ChannelMessage::ChanAftertouch {
            ch,
            pressure: data_u7(data, 0)?,
        },
        0xe0 => {
            let value = u16::from(data_u7(data, 0)?.0) | (u16::from(data_u7(data, 1)?.0) << 7);
            ChannelMessage::PitchBend {
                ch,
                value: U14::try_from(value)?,
            }
        }
        _ => return Err(MidiError::NotChannelStatus(status)),
    };
    Ok(message)
}

/// Read the 7-bit data byte at `index`, failing closed on a short slice or an
/// out-of-range (`> 127`) byte.
fn data_u7(data: &[u8], index: usize) -> Result<U7, MidiError> {
    let byte = *data.get(index).ok_or(MidiError::TruncatedChannel)?;
    U7::try_from(u16::from(byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(value: u8) -> Channel {
        Channel::new(value).expect("channel in range")
    }

    /// One representative of every [`ChannelMessage`] variant, on assorted
    /// channels and with boundary data values.
    fn every_variant() -> Vec<ChannelMessage> {
        vec![
            ChannelMessage::NoteOff {
                ch: ch(0),
                key: U7(60),
                vel: U7(0),
            },
            ChannelMessage::NoteOn {
                ch: ch(15),
                key: U7(127),
                vel: U7(100),
            },
            ChannelMessage::PolyAftertouch {
                ch: ch(3),
                key: U7(64),
                pressure: U7(90),
            },
            ChannelMessage::ControlChange {
                ch: ch(9),
                cc: U7(7),
                value: U7(127),
            },
            ChannelMessage::ProgramChange {
                ch: ch(1),
                program: U7(42),
            },
            ChannelMessage::ChanAftertouch {
                ch: ch(12),
                pressure: U7(55),
            },
            ChannelMessage::PitchBend {
                ch: ch(0),
                value: U14(0),
            },
            ChannelMessage::PitchBend {
                ch: ch(7),
                value: U14(8192),
            },
            ChannelMessage::PitchBend {
                ch: ch(15),
                value: U14(16_383),
            },
        ]
    }

    #[test]
    fn encode_then_decode_round_trips_every_variant() {
        for message in every_variant() {
            let (status, data) = encode_channel(&message);
            let decoded = decode_channel(status, &data).expect("decode");
            assert_eq!(decoded, message, "round trip for {message:?}");
        }
    }

    #[test]
    fn non_channel_status_is_rejected() {
        assert_eq!(
            decode_channel(0xf0, &[0x00]),
            Err(MidiError::NotChannelStatus(0xf0))
        );
    }

    #[test]
    fn truncated_data_is_rejected() {
        // NoteOn needs two data bytes.
        assert_eq!(
            decode_channel(0x90, &[60]),
            Err(MidiError::TruncatedChannel)
        );
        // ProgramChange needs one.
        assert_eq!(decode_channel(0xc0, &[]), Err(MidiError::TruncatedChannel));
    }

    #[test]
    fn data_byte_above_7_bits_is_rejected() {
        assert_eq!(
            decode_channel(0x90, &[200, 0]),
            Err(MidiError::InvalidU7(200))
        );
    }
}
