#![forbid(unsafe_code)]

use std::convert::TryFrom;

use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaBucket, MetaEvent, MidiEvent, MidiPayload, RawBytes, SysExEvent,
    TickTime, U7, U14, synthetic_origin,
};

use crate::{
    SmfDivision, SmfError, SmfFile, SmfFormat, SmfLimitKind, SmfReadLimits, SmfTrack, SmpteRate,
    decode_vlq_at,
    limits::{ReadBudget, enforce_limit},
};

/// Parses a complete Standard MIDI File from `bytes` using defensive default
/// resource bounds.
pub fn read_smf(bytes: &[u8]) -> Result<SmfFile, SmfError> {
    read_smf_with_limits(bytes, SmfReadLimits::default())
}

/// Parses a complete Standard MIDI File from `bytes` under explicit resource
/// bounds.
///
/// All declared sizes and counts are checked before allocation or payload
/// copying. Errors retain absolute byte offsets, including errors inside track
/// chunks and variable-length quantities.
pub fn read_smf_with_limits(bytes: &[u8], limits: SmfReadLimits) -> Result<SmfFile, SmfError> {
    enforce_limit(
        0,
        SmfLimitKind::FileBytes,
        bytes.len(),
        limits.max_file_bytes,
    )?;

    let mut pos = 0usize;
    if read_exact(bytes, &mut pos, 4)? != b"MThd" {
        return Err(SmfError::InvalidHeader { offset: 0 });
    }
    let header_len = read_u32(bytes, &mut pos)? as usize;
    if header_len < 6 {
        return Err(SmfError::InvalidHeader { offset: 4 });
    }
    enforce_limit(
        4,
        SmfLimitKind::HeaderBytes,
        header_len,
        limits.max_header_bytes,
    )?;
    let header_start = pos;
    let header_end = header_start
        .checked_add(header_len)
        .ok_or(SmfError::UnexpectedEof {
            offset: header_start,
        })?;
    if header_end > bytes.len() {
        return Err(SmfError::UnexpectedEof {
            offset: bytes.len(),
        });
    }
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
    let track_count_offset = pos;
    let track_count = read_u16(bytes, &mut pos)? as usize;
    enforce_limit(
        track_count_offset,
        SmfLimitKind::TrackCount,
        track_count,
        limits.max_tracks,
    )?;
    validate_format_tracks(format, track_count)?;
    let division_offset = pos;
    let division = decode_division(read_u16(bytes, &mut pos)?, division_offset)?;
    pos = header_end;

    let mut tracks = Vec::new();
    tracks
        .try_reserve_exact(track_count)
        .map_err(|_| SmfError::AllocationFailed {
            offset: track_count_offset,
            requested: track_count,
        })?;
    let mut budget = ReadBudget::new(limits);
    for _ in 0..track_count {
        let chunk_offset = pos;
        if read_exact(bytes, &mut pos, 4)? != b"MTrk" {
            return Err(SmfError::InvalidHeader {
                offset: chunk_offset,
            });
        }
        let track_len_offset = pos;
        let track_len = read_u32(bytes, &mut pos)? as usize;
        enforce_limit(
            track_len_offset,
            SmfLimitKind::TrackBytes,
            track_len,
            limits.max_track_bytes,
        )?;
        let track_end = pos
            .checked_add(track_len)
            .ok_or(SmfError::UnexpectedEof { offset: pos })?;
        if track_end > bytes.len() {
            return Err(SmfError::UnexpectedEof {
                offset: bytes.len(),
            });
        }
        tracks.push(read_track(
            &bytes[pos..track_end],
            pos,
            division.event_time_base(),
            &mut budget,
        )?);
        pos = track_end;
    }
    if pos != bytes.len() {
        return Err(SmfError::InvalidHeader { offset: pos });
    }
    Ok(SmfFile {
        format,
        division,
        tracks,
    })
}

fn decode_division(raw: u16, offset: usize) -> Result<SmfDivision, SmfError> {
    let [high, low] = raw.to_be_bytes();
    if high & 0x80 == 0 {
        return SmfDivision::metrical(raw).ok_or(SmfError::InvalidDivision { offset, raw });
    }
    let frames_per_second =
        SmpteRate::from_header_byte(high).ok_or(SmfError::InvalidDivision { offset, raw })?;
    SmfDivision::smpte(frames_per_second, low).ok_or(SmfError::InvalidDivision { offset, raw })
}

fn validate_format_tracks(format: SmfFormat, track_count: usize) -> Result<(), SmfError> {
    if track_count == 0 || (format == SmfFormat::SingleTrack && track_count != 1) {
        return Err(SmfError::FormatTrackMismatch);
    }
    Ok(())
}

fn read_track(
    bytes: &[u8],
    base_offset: usize,
    time_base: u32,
    budget: &mut ReadBudget,
) -> Result<SmfTrack, SmfError> {
    let mut pos = 0usize;
    let mut abs_ticks = 0i64;
    let mut events = Vec::new();
    let mut running_status: Option<u8> = None;
    let mut saw_end_of_track = false;
    while pos < bytes.len() {
        if saw_end_of_track {
            return Err(SmfError::InvalidHeader {
                offset: base_offset + pos,
            });
        }
        let delta_offset = base_offset + pos;
        budget.claim_event(delta_offset)?;
        events
            .try_reserve(1)
            .map_err(|_| SmfError::AllocationFailed {
                offset: delta_offset,
                requested: events.len().saturating_add(1),
            })?;
        let delta = decode_vlq_at(bytes, &mut pos, base_offset)?;
        abs_ticks = abs_ticks
            .checked_add(i64::from(delta))
            .ok_or(SmfError::TimeOverflow {
                offset: delta_offset,
            })?;
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
                    decode_meta(bytes, base_offset, &mut pos, budget)?
                }
                0xf0 | 0xf7 => {
                    running_status = None;
                    decode_sysex(bytes, base_offset, &mut pos, status_or_data, budget)?
                }
                status => {
                    let payload = decode_system(bytes, base_offset, &mut pos, status)?;
                    if !is_realtime_status(status) {
                        running_status = None;
                    }
                    payload
                }
            }
        };
        saw_end_of_track = matches!(payload, MidiPayload::Meta(MetaEvent::EndOfTrack));
        events.push(MidiEvent {
            time: TickTime::new(abs_ticks, time_base).map_err(|_| SmfError::InexactEventTime)?,
            origin: synthetic_origin(),
            payload,
        });
    }
    if !saw_end_of_track {
        return Err(SmfError::MissingEndOfTrack {
            offset: base_offset + bytes.len(),
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

fn decode_meta(
    bytes: &[u8],
    base_offset: usize,
    pos: &mut usize,
    budget: &mut ReadBudget,
) -> Result<MidiPayload, SmfError> {
    let type_offset = base_offset + *pos;
    let type_byte = *bytes.get(*pos).ok_or(SmfError::UnexpectedEof {
        offset: type_offset,
    })?;
    *pos += 1;
    let len_offset = base_offset + *pos;
    let len = decode_vlq_at(bytes, pos, base_offset)? as usize;
    if let Some(expected) = fixed_meta_len(type_byte)
        && len != expected
    {
        return Err(SmfError::InvalidMetaLength {
            offset: type_offset,
            type_byte,
            expected,
            actual: len,
        });
    }
    let data = read_payload(bytes, base_offset, pos, len, len_offset, budget)?;
    let event = match type_byte {
        0x2f => MetaEvent::EndOfTrack,
        0x51 => MetaEvent::Tempo {
            us_per_quarter: (u32::from(data[0]) << 16)
                | (u32::from(data[1]) << 8)
                | u32::from(data[2]),
        },
        0x58 => MetaEvent::TimeSig {
            num: data[0],
            den_pow2: data[1],
            clocks_per_click: data[2],
            thirty_seconds_per_quarter: data[3],
        },
        0x59 => MetaEvent::KeySig {
            sharps_flats: data[0] as i8,
            minor: data[1] != 0,
        },
        _ => MetaEvent::Other(MetaBucket { type_byte, data }),
    };
    Ok(MidiPayload::Meta(event))
}

fn fixed_meta_len(type_byte: u8) -> Option<usize> {
    match type_byte {
        0x00 => Some(2),
        0x20 => Some(1),
        0x21 => Some(1),
        0x2f => Some(0),
        0x51 => Some(3),
        0x54 => Some(5),
        0x58 => Some(4),
        0x59 => Some(2),
        _ => None,
    }
}

fn decode_sysex(
    bytes: &[u8],
    base_offset: usize,
    pos: &mut usize,
    status: u8,
    budget: &mut ReadBudget,
) -> Result<MidiPayload, SmfError> {
    let len_offset = base_offset + *pos;
    let len = decode_vlq_at(bytes, pos, base_offset)? as usize;
    let data = read_payload(bytes, base_offset, pos, len, len_offset, budget)?;
    let event = match status {
        0xf0 => SysExEvent::F0 { data },
        0xf7 => SysExEvent::F7 { data },
        _ => unreachable!(),
    };
    Ok(MidiPayload::SysEx(event))
}

fn decode_system(
    bytes: &[u8],
    base_offset: usize,
    pos: &mut usize,
    status: u8,
) -> Result<MidiPayload, SmfError> {
    let status_offset = base_offset + pos.saturating_sub(1);
    let data_len = system_data_len(status).ok_or(SmfError::UnsupportedStatus {
        offset: status_offset,
        status,
    })?;
    let mut data = Vec::new();
    data.try_reserve_exact(data_len)
        .map_err(|_| SmfError::AllocationFailed {
            offset: status_offset,
            requested: data_len,
        })?;
    for _ in 0..data_len {
        data.push(
            read_data_byte(bytes, base_offset, pos).map_err(|error| match error {
                SmfError::InvalidChannelData { offset } => {
                    SmfError::InvalidSystemEvent { offset, status }
                }
                other => other,
            })?,
        );
    }
    Ok(MidiPayload::Raw(RawBytes { status, data }))
}

pub(crate) const fn system_data_len(status: u8) -> Option<usize> {
    match status {
        0xf1 | 0xf3 => Some(1),
        0xf2 => Some(2),
        0xf6 | 0xf8 | 0xfa | 0xfb | 0xfc | 0xfe => Some(0),
        _ => None,
    }
}

pub(crate) const fn is_realtime_status(status: u8) -> bool {
    matches!(status, 0xf8 | 0xfa | 0xfb | 0xfc | 0xfe)
}

fn read_payload(
    bytes: &[u8],
    base_offset: usize,
    pos: &mut usize,
    len: usize,
    len_offset: usize,
    budget: &mut ReadBudget,
) -> Result<Vec<u8>, SmfError> {
    budget.claim_payload(len_offset, len)?;
    let source = read_exact_at(bytes, pos, len, base_offset)?;
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| SmfError::AllocationFailed {
            offset: len_offset,
            requested: len,
        })?;
    data.extend_from_slice(source);
    Ok(data)
}

fn read_exact<'a>(bytes: &'a [u8], pos: &mut usize, len: usize) -> Result<&'a [u8], SmfError> {
    read_exact_at(bytes, pos, len, 0)
}

fn read_exact_at<'a>(
    bytes: &'a [u8],
    pos: &mut usize,
    len: usize,
    base_offset: usize,
) -> Result<&'a [u8], SmfError> {
    let end = pos.checked_add(len).ok_or(SmfError::UnexpectedEof {
        offset: base_offset + *pos,
    })?;
    let slice = bytes.get(*pos..end).ok_or(SmfError::UnexpectedEof {
        offset: base_offset + bytes.len(),
    })?;
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
