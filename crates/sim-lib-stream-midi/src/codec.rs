use sim_kernel::{Error, Result};
use sim_lib_midi_core::{MetaBucket, MetaEvent, MidiPayload, RawBytes, SysExEvent, wire};

pub(crate) fn payload_to_bytes(payload: &MidiPayload) -> Vec<u8> {
    match payload {
        MidiPayload::Channel(message) => {
            let (status, data) = wire::encode_channel(message);
            prefixed(status, &data)
        }
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
        0x80..=0xef => Ok(MidiPayload::Channel(
            wire::decode_channel(status, data).map_err(|err| Error::Eval(err.to_string()))?,
        )),
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
