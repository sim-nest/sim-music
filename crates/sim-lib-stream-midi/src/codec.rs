use sim_kernel::{Error, Result};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaBucket, MetaEvent, MidiPayload, RawBytes, SysExEvent, U7, U14,
};

pub(crate) fn payload_to_bytes(payload: &MidiPayload) -> Vec<u8> {
    match payload {
        MidiPayload::Channel(message) => channel_message_to_bytes(*message),
        MidiPayload::Meta(event) => meta_event_to_bytes(event),
        MidiPayload::SysEx(SysExEvent::F0 { data }) => prefixed(0xf0, data),
        MidiPayload::SysEx(SysExEvent::F7 { data }) => prefixed(0xf7, data),
        MidiPayload::Raw(raw) => prefixed(raw.status, &raw.data),
    }
}

pub(crate) fn bytes_to_payload(bytes: &[u8]) -> Result<MidiPayload> {
    let Some((&status, data)) = bytes.split_first() else {
        return Err(Error::Eval(
            "MIDI packet event bytes must not be empty".to_owned(),
        ));
    };
    match status {
        0x80..=0xef => channel_message_from_bytes(status, data),
        0xf0 => Ok(MidiPayload::SysEx(SysExEvent::F0 {
            data: data.to_vec(),
        })),
        0xf7 => Ok(MidiPayload::SysEx(SysExEvent::F7 {
            data: data.to_vec(),
        })),
        0xff => meta_event_from_bytes(data),
        _ => Ok(MidiPayload::Raw(RawBytes {
            status,
            data: data.to_vec(),
        })),
    }
}

fn channel_message_to_bytes(message: ChannelMessage) -> Vec<u8> {
    match message {
        ChannelMessage::NoteOff { ch, key, vel } => vec![0x80 | ch.0, key.0, vel.0],
        ChannelMessage::NoteOn { ch, key, vel } => vec![0x90 | ch.0, key.0, vel.0],
        ChannelMessage::PolyAftertouch { ch, key, pressure } => {
            vec![0xa0 | ch.0, key.0, pressure.0]
        }
        ChannelMessage::ControlChange { ch, cc, value } => vec![0xb0 | ch.0, cc.0, value.0],
        ChannelMessage::ProgramChange { ch, program } => vec![0xc0 | ch.0, program.0],
        ChannelMessage::ChanAftertouch { ch, pressure } => vec![0xd0 | ch.0, pressure.0],
        ChannelMessage::PitchBend { ch, value } => {
            vec![0xe0 | ch.0, (value.0 & 0x7f) as u8, (value.0 >> 7) as u8]
        }
    }
}

fn channel_message_from_bytes(status: u8, data: &[u8]) -> Result<MidiPayload> {
    let ch = Channel::new(status & 0x0f)
        .map_err(|err| Error::Eval(format!("invalid MIDI channel: {err}")))?;
    let message = match status & 0xf0 {
        0x80 => ChannelMessage::NoteOff {
            ch,
            key: u7(data, 0)?,
            vel: u7(data, 1)?,
        },
        0x90 => ChannelMessage::NoteOn {
            ch,
            key: u7(data, 0)?,
            vel: u7(data, 1)?,
        },
        0xa0 => ChannelMessage::PolyAftertouch {
            ch,
            key: u7(data, 0)?,
            pressure: u7(data, 1)?,
        },
        0xb0 => ChannelMessage::ControlChange {
            ch,
            cc: u7(data, 0)?,
            value: u7(data, 1)?,
        },
        0xc0 => ChannelMessage::ProgramChange {
            ch,
            program: u7(data, 0)?,
        },
        0xd0 => ChannelMessage::ChanAftertouch {
            ch,
            pressure: u7(data, 0)?,
        },
        0xe0 => {
            let value = u16::from(u7(data, 0)?.0) | (u16::from(u7(data, 1)?.0) << 7);
            ChannelMessage::PitchBend {
                ch,
                value: U14::try_from(value)
                    .map_err(|err| Error::Eval(format!("invalid pitch bend value: {err}")))?,
            }
        }
        _ => {
            return Ok(MidiPayload::Raw(RawBytes {
                status,
                data: data.to_vec(),
            }));
        }
    };
    Ok(MidiPayload::Channel(message))
}

fn meta_event_to_bytes(event: &MetaEvent) -> Vec<u8> {
    match event {
        MetaEvent::EndOfTrack => vec![0xff, 0x2f],
        MetaEvent::Tempo { us_per_quarter } => {
            let bytes = us_per_quarter.to_be_bytes();
            vec![0xff, 0x51, bytes[1], bytes[2], bytes[3]]
        }
        MetaEvent::TimeSig {
            num,
            den_pow2,
            clocks_per_click,
            thirty_seconds_per_quarter,
        } => vec![
            0xff,
            0x58,
            *num,
            *den_pow2,
            *clocks_per_click,
            *thirty_seconds_per_quarter,
        ],
        MetaEvent::KeySig {
            sharps_flats,
            minor,
        } => vec![0xff, 0x59, *sharps_flats as u8, u8::from(*minor)],
        MetaEvent::Other(bucket) => {
            let mut out = vec![0xff, bucket.type_byte];
            out.extend_from_slice(&bucket.data);
            out
        }
    }
}

fn meta_event_from_bytes(data: &[u8]) -> Result<MidiPayload> {
    let Some((&kind, payload)) = data.split_first() else {
        return Err(Error::Eval(
            "MIDI meta packet bytes missing type byte".to_owned(),
        ));
    };
    let event = match kind {
        0x2f if payload.is_empty() => MetaEvent::EndOfTrack,
        0x51 if payload.len() == 3 => MetaEvent::Tempo {
            us_per_quarter: u32::from_be_bytes([0, payload[0], payload[1], payload[2]]),
        },
        0x58 if payload.len() == 4 => MetaEvent::TimeSig {
            num: payload[0],
            den_pow2: payload[1],
            clocks_per_click: payload[2],
            thirty_seconds_per_quarter: payload[3],
        },
        0x59 if payload.len() == 2 => MetaEvent::KeySig {
            sharps_flats: payload[0] as i8,
            minor: payload[1] != 0,
        },
        _ => MetaEvent::Other(MetaBucket {
            type_byte: kind,
            data: payload.to_vec(),
        }),
    };
    Ok(MidiPayload::Meta(event))
}

fn prefixed(status: u8, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 1);
    out.push(status);
    out.extend_from_slice(data);
    out
}

fn u7(data: &[u8], index: usize) -> Result<U7> {
    let value = *data
        .get(index)
        .ok_or_else(|| Error::Eval("MIDI channel packet bytes are truncated".to_owned()))?;
    U7::try_from(u16::from(value)).map_err(|err| Error::Eval(format!("invalid U7 byte: {err}")))
}
