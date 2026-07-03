#![forbid(unsafe_code)]

use std::convert::TryFrom;

use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaBucket, MetaEvent, MidiEvent, MidiPayload, SysExEvent, TickTime,
    U7, U14, synthetic_origin,
};

use crate::{SmfError, SmfFile, SmfFormat, SmfTrack, decode_vlq_at};

/// Parses a complete Standard MIDI File from `bytes` into an [`SmfFile`].
///
/// Returns an [`SmfError`] for a malformed header, truncated input, SMPTE
/// division, or an unsupported event.
pub fn read_smf(bytes: &[u8]) -> Result<SmfFile, SmfError> {
    let mut pos = 0usize;
    if read_exact(bytes, &mut pos, 4)? != b"MThd" {
        return Err(SmfError::InvalidHeader { offset: 0 });
    }
    let header_len = read_u32(bytes, &mut pos)? as usize;
    if header_len < 6 {
        return Err(SmfError::InvalidHeader { offset: 4 });
    }
    let header_start = pos;
    let format = match read_u16(bytes, &mut pos)? {
        0 => SmfFormat::SingleTrack,
        1 => SmfFormat::Simultaneous,
        2 => SmfFormat::Independent,
        _ => {
            return Err(SmfError::InvalidHeader {
                offset: header_start,
            });
        }
    };
    let track_count = read_u16(bytes, &mut pos)? as usize;
    let division_offset = pos;
    let division = read_u16(bytes, &mut pos)?;
    if division & 0x8000 != 0 {
        return Err(SmfError::UnsupportedSmpteDivision {
            offset: division_offset,
            raw: division,
        });
    }
    let tpq = u32::from(division);
    pos = header_start + header_len;
    let mut tracks = Vec::with_capacity(track_count);
    for _ in 0..track_count {
        if read_exact(bytes, &mut pos, 4)? != b"MTrk" {
            return Err(SmfError::InvalidHeader {
                offset: pos.saturating_sub(4),
            });
        }
        let track_len = read_u32(bytes, &mut pos)? as usize;
        let track_end = pos
            .checked_add(track_len)
            .ok_or(SmfError::UnexpectedEof { offset: pos })?;
        if track_end > bytes.len() {
            return Err(SmfError::UnexpectedEof { offset: pos });
        }
        tracks.push(read_track(&bytes[pos..track_end], pos, tpq)?);
        pos = track_end;
    }
    Ok(SmfFile {
        format,
        tpq,
        tracks,
    })
}

fn read_track(bytes: &[u8], base_offset: usize, tpq: u32) -> Result<SmfTrack, SmfError> {
    let mut pos = 0usize;
    let mut abs_ticks = 0i64;
    let mut events = Vec::new();
    let mut running_status: Option<u8> = None;
    while pos < bytes.len() {
        abs_ticks += i64::from(decode_vlq_at(bytes, &mut pos)?);
        let event_offset = base_offset + pos;
        let status_or_data = *bytes.get(pos).ok_or(SmfError::UnexpectedEof {
            offset: event_offset,
        })?;
        pos += 1;
        let payload = if status_or_data < 0x80 {
            let status = running_status.ok_or(SmfError::MalformedRunningStatus {
                offset: event_offset,
            })?;
            decode_channel(bytes, base_offset, &mut pos, status, Some(status_or_data))?
        } else {
            match status_or_data {
                0x80..=0xef => {
                    running_status = Some(status_or_data);
                    decode_channel(bytes, base_offset, &mut pos, status_or_data, None)?
                }
                0xff => {
                    running_status = None;
                    decode_meta(bytes, base_offset, &mut pos)?
                }
                0xf0 | 0xf7 => {
                    running_status = None;
                    decode_sysex(bytes, base_offset, &mut pos, status_or_data)?
                }
                status => {
                    return Err(SmfError::UnsupportedStatus {
                        offset: event_offset,
                        status,
                    });
                }
            }
        };
        events.push(MidiEvent {
            time: TickTime::new(abs_ticks, tpq).map_err(|_| SmfError::InexactEventTime)?,
            origin: synthetic_origin(),
            payload,
        });
    }
    Ok(SmfTrack { events })
}

fn decode_channel(
    bytes: &[u8],
    base_offset: usize,
    pos: &mut usize,
    status: u8,
    first_data: Option<u8>,
) -> Result<MidiPayload, SmfError> {
    let ch = Channel::new(status & 0x0f).map_err(|_| SmfError::InvalidChannelData {
        offset: base_offset + pos.saturating_sub(1),
    })?;
    let kind = status >> 4;
    let data_len = match kind {
        0x8 | 0x9 | 0xa | 0xb | 0xe => 2,
        0xc | 0xd => 1,
        _ => {
            return Err(SmfError::UnsupportedStatus {
                offset: base_offset + pos.saturating_sub(1),
                status,
            });
        }
    };
    let first = match first_data {
        Some(value) => value,
        None => read_data_byte(bytes, base_offset, pos)?,
    };
    let second = if data_len == 2 {
        Some(read_data_byte(bytes, base_offset, pos)?)
    } else {
        None
    };
    let first = to_u7(first, base_offset + pos.saturating_sub(data_len))?;
    let payload = match kind {
        0x8 => MidiPayload::Channel(ChannelMessage::NoteOff {
            ch,
            key: first,
            vel: to_u7(
                second.expect("two-byte event"),
                base_offset + pos.saturating_sub(1),
            )?,
        }),
        0x9 => MidiPayload::Channel(ChannelMessage::NoteOn {
            ch,
            key: first,
            vel: to_u7(
                second.expect("two-byte event"),
                base_offset + pos.saturating_sub(1),
            )?,
        }),
        0xa => MidiPayload::Channel(ChannelMessage::PolyAftertouch {
            ch,
            key: first,
            pressure: to_u7(
                second.expect("two-byte event"),
                base_offset + pos.saturating_sub(1),
            )?,
        }),
        0xb => MidiPayload::Channel(ChannelMessage::ControlChange {
            ch,
            cc: first,
            value: to_u7(
                second.expect("two-byte event"),
                base_offset + pos.saturating_sub(1),
            )?,
        }),
        0xc => MidiPayload::Channel(ChannelMessage::ProgramChange { ch, program: first }),
        0xd => MidiPayload::Channel(ChannelMessage::ChanAftertouch {
            ch,
            pressure: first,
        }),
        0xe => {
            let lsb = u16::from(first.0);
            let msb = u16::from(second.expect("two-byte event"));
            let value =
                U14::try_from(lsb | (msb << 7)).map_err(|_| SmfError::InvalidChannelData {
                    offset: base_offset + *pos,
                })?;
            MidiPayload::Channel(ChannelMessage::PitchBend { ch, value })
        }
        _ => unreachable!(),
    };
    Ok(payload)
}

fn decode_meta(bytes: &[u8], base_offset: usize, pos: &mut usize) -> Result<MidiPayload, SmfError> {
    let type_offset = base_offset + *pos;
    let type_byte = *bytes.get(*pos).ok_or(SmfError::UnexpectedEof {
        offset: type_offset,
    })?;
    *pos += 1;
    let len = decode_vlq_at(bytes, pos)? as usize;
    let data = read_exact(bytes, pos, len)?.to_vec();
    let event = match type_byte {
        0x2f if data.is_empty() => MetaEvent::EndOfTrack,
        0x51 if data.len() == 3 => MetaEvent::Tempo {
            us_per_quarter: (u32::from(data[0]) << 16)
                | (u32::from(data[1]) << 8)
                | u32::from(data[2]),
        },
        0x58 if data.len() == 4 => MetaEvent::TimeSig {
            num: data[0],
            den_pow2: data[1],
            clocks_per_click: data[2],
            thirty_seconds_per_quarter: data[3],
        },
        0x59 if data.len() == 2 => MetaEvent::KeySig {
            sharps_flats: data[0] as i8,
            minor: data[1] != 0,
        },
        _ => MetaEvent::Other(MetaBucket { type_byte, data }),
    };
    Ok(MidiPayload::Meta(event))
}

fn decode_sysex(
    bytes: &[u8],
    base_offset: usize,
    pos: &mut usize,
    status: u8,
) -> Result<MidiPayload, SmfError> {
    let len = decode_vlq_at(bytes, pos)? as usize;
    let data = read_exact(bytes, pos, len)?.to_vec();
    let event = match status {
        0xf0 => SysExEvent::F0 { data },
        0xf7 => SysExEvent::F7 { data },
        _ => unreachable!(),
    };
    let _ = base_offset;
    Ok(MidiPayload::SysEx(event))
}

fn read_exact<'a>(bytes: &'a [u8], pos: &mut usize, len: usize) -> Result<&'a [u8], SmfError> {
    let end = pos
        .checked_add(len)
        .ok_or(SmfError::UnexpectedEof { offset: *pos })?;
    let slice = bytes
        .get(*pos..end)
        .ok_or(SmfError::UnexpectedEof { offset: *pos })?;
    *pos = end;
    Ok(slice)
}

fn read_u16(bytes: &[u8], pos: &mut usize) -> Result<u16, SmfError> {
    let raw = read_exact(bytes, pos, 2)?;
    Ok(u16::from_be_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32, SmfError> {
    let raw = read_exact(bytes, pos, 4)?;
    Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn read_data_byte(bytes: &[u8], base_offset: usize, pos: &mut usize) -> Result<u8, SmfError> {
    let offset = base_offset + *pos;
    let byte = *bytes.get(*pos).ok_or(SmfError::UnexpectedEof { offset })?;
    if byte >= 0x80 {
        return Err(SmfError::InvalidChannelData { offset });
    }
    *pos += 1;
    Ok(byte)
}

fn to_u7(value: u8, offset: usize) -> Result<U7, SmfError> {
    U7::try_from(u16::from(value)).map_err(|_| SmfError::InvalidChannelData { offset })
}
