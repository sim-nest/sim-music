use std::convert::TryFrom;

use thiserror::Error;

use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaBucket, MetaEvent, MidiEvent, MidiPayload, RawBytes, SysExEvent,
    TickTime, U7, U14, synthetic_origin,
};
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTrack};

/// Errors raised while decoding a MIDI string shape, named by the shape that
/// failed to parse.
#[derive(Debug, Error, Clone)]
pub enum MidiShapeError {
    /// The input was not a valid `TickTime` form.
    #[error("invalid TickTime shape")]
    InvalidTickTime,
    /// The input was not a valid `MidiEvent` form.
    #[error("invalid MidiEvent shape")]
    InvalidMidiEvent,
    /// The input was not a recognised payload form.
    #[error("invalid MidiPayload shape")]
    InvalidMidiPayload,
    /// The input was not a valid channel-message form.
    #[error("invalid channel message shape")]
    InvalidChannelMessage,
    /// The input was not a valid meta-event form.
    #[error("invalid meta event shape")]
    InvalidMeta,
    /// The input was not a valid SysEx form.
    #[error("invalid sysex shape")]
    InvalidSysEx,
    /// The input was not a valid raw-bytes form.
    #[error("invalid raw bytes shape")]
    InvalidRaw,
    /// The input was not a valid SMF-track form.
    #[error("invalid SMF track shape")]
    InvalidSmfTrack,
    /// The input was not a valid SMF-file form.
    #[error("invalid SMF file shape")]
    InvalidSmfFile,
}

/// Encodes a [`TickTime`] as `#(TickTime <ticks> <tpq>)`.
pub fn encode_tick_time(time: TickTime) -> String {
    format!("#(TickTime {} {})", time.ticks, time.tpq)
}

/// Decodes a [`TickTime`] from its `#(TickTime ...)` form.
pub fn decode_tick_time(value: &str) -> Result<TickTime, MidiShapeError> {
    decode_tick_time_with_tpq(value, None)
}

/// Decodes a [`TickTime`], also accepting the `Nq` / `N/Dq` quarter-note reader
/// sugar resolved against `inherited_tpq`.
pub fn decode_tick_time_with_tpq(
    value: &str,
    inherited_tpq: Option<u32>,
) -> Result<TickTime, MidiShapeError> {
    if let Some(inner) = value.strip_suffix('q') {
        let tpq = inherited_tpq.ok_or(MidiShapeError::InvalidTickTime)?;
        let (numerator, denominator) = parse_ratio(inner)?;
        let ticks = numerator
            .checked_mul(i64::from(tpq))
            .ok_or(MidiShapeError::InvalidTickTime)?
            / denominator;
        return TickTime::new(ticks, tpq).map_err(|_| MidiShapeError::InvalidTickTime);
    }
    let inner = value
        .strip_prefix("#(TickTime ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidTickTime)?;
    let mut parts = inner.split_whitespace();
    let ticks = parts
        .next()
        .ok_or(MidiShapeError::InvalidTickTime)?
        .parse::<i64>()
        .map_err(|_| MidiShapeError::InvalidTickTime)?;
    let tpq = parts
        .next()
        .ok_or(MidiShapeError::InvalidTickTime)?
        .parse::<u32>()
        .map_err(|_| MidiShapeError::InvalidTickTime)?;
    TickTime::new(ticks, tpq).map_err(|_| MidiShapeError::InvalidTickTime)
}

/// Encodes a [`ChannelMessage`] as a `#(Channel ...)` form.
pub fn encode_channel_message(message: ChannelMessage) -> String {
    match message {
        ChannelMessage::NoteOff { ch, key, vel } => {
            format!("#(Channel NoteOff {} {} {})", ch.0, key.0, vel.0)
        }
        ChannelMessage::NoteOn { ch, key, vel } => {
            format!("#(Channel NoteOn {} {} {})", ch.0, key.0, vel.0)
        }
        ChannelMessage::PolyAftertouch { ch, key, pressure } => {
            format!(
                "#(Channel PolyAftertouch {} {} {})",
                ch.0, key.0, pressure.0
            )
        }
        ChannelMessage::ControlChange { ch, cc, value } => {
            format!("#(Channel ControlChange {} {} {})", ch.0, cc.0, value.0)
        }
        ChannelMessage::ProgramChange { ch, program } => {
            format!("#(Channel ProgramChange {} {})", ch.0, program.0)
        }
        ChannelMessage::ChanAftertouch { ch, pressure } => {
            format!("#(Channel ChanAftertouch {} {})", ch.0, pressure.0)
        }
        ChannelMessage::PitchBend { ch, value } => {
            format!("#(Channel PitchBend {} {})", ch.0, value.0)
        }
    }
}

/// Decodes a [`ChannelMessage`] from a `#(Channel ...)` form.
pub fn decode_channel_message(value: &str) -> Result<ChannelMessage, MidiShapeError> {
    let inner = value
        .strip_prefix("#(Channel ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidChannelMessage)?;
    let parts = inner.split_whitespace().collect::<Vec<_>>();
    let kind = *parts.first().ok_or(MidiShapeError::InvalidChannelMessage)?;
    match kind {
        "NoteOff" => Ok(ChannelMessage::NoteOff {
            ch: parse_channel(parts.get(1))?,
            key: parse_u7(parts.get(2))?,
            vel: parse_u7(parts.get(3))?,
        }),
        "NoteOn" => Ok(ChannelMessage::NoteOn {
            ch: parse_channel(parts.get(1))?,
            key: parse_u7(parts.get(2))?,
            vel: parse_u7(parts.get(3))?,
        }),
        "PolyAftertouch" => Ok(ChannelMessage::PolyAftertouch {
            ch: parse_channel(parts.get(1))?,
            key: parse_u7(parts.get(2))?,
            pressure: parse_u7(parts.get(3))?,
        }),
        "ControlChange" => Ok(ChannelMessage::ControlChange {
            ch: parse_channel(parts.get(1))?,
            cc: parse_u7(parts.get(2))?,
            value: parse_u7(parts.get(3))?,
        }),
        "ProgramChange" => Ok(ChannelMessage::ProgramChange {
            ch: parse_channel(parts.get(1))?,
            program: parse_u7(parts.get(2))?,
        }),
        "ChanAftertouch" => Ok(ChannelMessage::ChanAftertouch {
            ch: parse_channel(parts.get(1))?,
            pressure: parse_u7(parts.get(2))?,
        }),
        "PitchBend" => Ok(ChannelMessage::PitchBend {
            ch: parse_channel(parts.get(1))?,
            value: parse_u14(parts.get(2))?,
        }),
        _ => Err(MidiShapeError::InvalidChannelMessage),
    }
}

/// Encodes a [`MetaEvent`] as a `#(Meta ...)` form.
pub fn encode_meta_event(event: &MetaEvent) -> String {
    match event {
        MetaEvent::EndOfTrack => "#(Meta EndOfTrack)".to_owned(),
        MetaEvent::Tempo { us_per_quarter } => format!("#(Meta Tempo {})", us_per_quarter),
        MetaEvent::TimeSig {
            num,
            den_pow2,
            clocks_per_click,
            thirty_seconds_per_quarter,
        } => format!(
            "#(Meta TimeSig {} {} {} {})",
            num, den_pow2, clocks_per_click, thirty_seconds_per_quarter
        ),
        MetaEvent::KeySig {
            sharps_flats,
            minor,
        } => format!("#(Meta KeySig {} {})", sharps_flats, u8::from(*minor)),
        MetaEvent::Other(bucket) => format!(
            "#(Meta Other {} {})",
            bucket.type_byte,
            encode_bytes(&bucket.data)
        ),
    }
}

/// Decodes a [`MetaEvent`] from a `#(Meta ...)` form.
pub fn decode_meta_event(value: &str) -> Result<MetaEvent, MidiShapeError> {
    let inner = value
        .strip_prefix("#(Meta ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidMeta)?;
    let parts = inner.split_whitespace().collect::<Vec<_>>();
    let kind = *parts.first().ok_or(MidiShapeError::InvalidMeta)?;
    match kind {
        "EndOfTrack" => Ok(MetaEvent::EndOfTrack),
        "Tempo" => Ok(MetaEvent::Tempo {
            us_per_quarter: parse_u32(parts.get(1), MidiShapeError::InvalidMeta)?,
        }),
        "TimeSig" => Ok(MetaEvent::TimeSig {
            num: parse_u8(parts.get(1), MidiShapeError::InvalidMeta)?,
            den_pow2: parse_u8(parts.get(2), MidiShapeError::InvalidMeta)?,
            clocks_per_click: parse_u8(parts.get(3), MidiShapeError::InvalidMeta)?,
            thirty_seconds_per_quarter: parse_u8(parts.get(4), MidiShapeError::InvalidMeta)?,
        }),
        "KeySig" => Ok(MetaEvent::KeySig {
            sharps_flats: parse_i8(parts.get(1), MidiShapeError::InvalidMeta)?,
            minor: parse_u8(parts.get(2), MidiShapeError::InvalidMeta)? != 0,
        }),
        "Other" => Ok(MetaEvent::Other(MetaBucket {
            type_byte: parse_u8(parts.get(1), MidiShapeError::InvalidMeta)?,
            data: decode_bytes(parts.get(2).ok_or(MidiShapeError::InvalidMeta)?)?,
        })),
        _ => Err(MidiShapeError::InvalidMeta),
    }
}

/// Encodes a [`SysExEvent`] as a `#(SysEx ...)` form with hex-encoded data.
pub fn encode_sysex(event: &SysExEvent) -> String {
    match event {
        SysExEvent::F0 { data } => format!("#(SysEx F0 {})", encode_bytes(data)),
        SysExEvent::F7 { data } => format!("#(SysEx F7 {})", encode_bytes(data)),
    }
}

/// Decodes a [`SysExEvent`] from a `#(SysEx ...)` form.
pub fn decode_sysex(value: &str) -> Result<SysExEvent, MidiShapeError> {
    let inner = value
        .strip_prefix("#(SysEx ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidSysEx)?;
    let parts = inner.split_whitespace().collect::<Vec<_>>();
    let kind = *parts.first().ok_or(MidiShapeError::InvalidSysEx)?;
    let data = decode_bytes(parts.get(1).ok_or(MidiShapeError::InvalidSysEx)?)?;
    match kind {
        "F0" => Ok(SysExEvent::F0 { data }),
        "F7" => Ok(SysExEvent::F7 { data }),
        _ => Err(MidiShapeError::InvalidSysEx),
    }
}

/// Encodes a [`RawBytes`] payload as a `#(Raw ...)` form.
pub fn encode_raw(raw: &RawBytes) -> String {
    format!("#(Raw {} {})", raw.status, encode_bytes(&raw.data))
}

/// Decodes a [`RawBytes`] payload from a `#(Raw ...)` form.
pub fn decode_raw(value: &str) -> Result<RawBytes, MidiShapeError> {
    let inner = value
        .strip_prefix("#(Raw ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidRaw)?;
    let parts = inner.split_whitespace().collect::<Vec<_>>();
    Ok(RawBytes {
        status: parse_u8(parts.first().copied(), MidiShapeError::InvalidRaw)?,
        data: decode_bytes(parts.get(1).ok_or(MidiShapeError::InvalidRaw)?)?,
    })
}

/// Encodes a [`MidiPayload`] using the matching family encoder.
pub fn encode_payload(payload: &MidiPayload) -> String {
    match payload {
        MidiPayload::Channel(message) => encode_channel_message(*message),
        MidiPayload::Meta(event) => encode_meta_event(event),
        MidiPayload::SysEx(event) => encode_sysex(event),
        MidiPayload::Raw(raw) => encode_raw(raw),
    }
}

/// Decodes a [`MidiPayload`], dispatching on the leading `#(Family ...)` tag.
pub fn decode_payload(value: &str) -> Result<MidiPayload, MidiShapeError> {
    if value.starts_with("#(Channel ") {
        return Ok(MidiPayload::Channel(decode_channel_message(value)?));
    }
    if value.starts_with("#(Meta ") {
        return Ok(MidiPayload::Meta(decode_meta_event(value)?));
    }
    if value.starts_with("#(SysEx ") {
        return Ok(MidiPayload::SysEx(decode_sysex(value)?));
    }
    if value.starts_with("#(Raw ") {
        return Ok(MidiPayload::Raw(decode_raw(value)?));
    }
    Err(MidiShapeError::InvalidMidiPayload)
}

/// Encodes a [`MidiEvent`] as `#(MidiEvent <time> <payload>)`.
pub fn encode_midi_event(event: &MidiEvent) -> String {
    format!(
        "#(MidiEvent {} {})",
        encode_tick_time(event.time),
        encode_payload(&event.payload)
    )
}

/// Decodes a [`MidiEvent`] from a `#(MidiEvent ...)` form, tagging it with a
/// synthetic origin.
pub fn decode_midi_event(value: &str) -> Result<MidiEvent, MidiShapeError> {
    let inner = value
        .strip_prefix("#(MidiEvent ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidMidiEvent)?;
    let split = split_top_level(inner);
    if split.len() != 2 {
        return Err(MidiShapeError::InvalidMidiEvent);
    }
    Ok(MidiEvent {
        time: decode_tick_time(split[0])?,
        origin: synthetic_origin(),
        payload: decode_payload(split[1])?,
    })
}

/// Encodes an [`SmfTrack`] as a `#(SmfTrack ...)` form.
pub fn encode_smf_track(track: &SmfTrack) -> String {
    if track.events.is_empty() {
        return "#(SmfTrack)".to_owned();
    }
    format!(
        "#(SmfTrack {})",
        track
            .events
            .iter()
            .map(encode_midi_event)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Decodes an [`SmfTrack`] from a `#(SmfTrack ...)` form.
pub fn decode_smf_track(value: &str) -> Result<SmfTrack, MidiShapeError> {
    let inner = value
        .strip_prefix("#(SmfTrack")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidSmfTrack)?
        .trim();
    if inner.is_empty() {
        return Ok(SmfTrack { events: Vec::new() });
    }
    let events = split_top_level(inner)
        .into_iter()
        .map(decode_midi_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SmfTrack { events })
}

/// Encodes an [`SmfFile`] as a `#(SmfFile <format> <tpq> ...)` form.
pub fn encode_smf_file(file: &SmfFile) -> String {
    let format = match file.format {
        SmfFormat::SingleTrack => "SingleTrack",
        SmfFormat::Simultaneous => "Simultaneous",
        SmfFormat::Independent => "Independent",
    };
    if file.tracks.is_empty() {
        return format!("#(SmfFile {} {})", format, file.tpq);
    }
    format!(
        "#(SmfFile {} {} {})",
        format,
        file.tpq,
        file.tracks
            .iter()
            .map(encode_smf_track)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Decodes an [`SmfFile`] from a `#(SmfFile ...)` form.
pub fn decode_smf_file(value: &str) -> Result<SmfFile, MidiShapeError> {
    let inner = value
        .strip_prefix("#(SmfFile ")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or(MidiShapeError::InvalidSmfFile)?;
    let parts = split_top_level(inner);
    if parts.len() < 2 {
        return Err(MidiShapeError::InvalidSmfFile);
    }
    let format = match parts[0] {
        "SingleTrack" => SmfFormat::SingleTrack,
        "Simultaneous" => SmfFormat::Simultaneous,
        "Independent" => SmfFormat::Independent,
        _ => return Err(MidiShapeError::InvalidSmfFile),
    };
    let tpq = parts[1]
        .parse::<u32>()
        .map_err(|_| MidiShapeError::InvalidSmfFile)?;
    let tracks = parts[2..]
        .iter()
        .map(|part| decode_smf_track(part))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SmfFile {
        format,
        tpq,
        tracks,
    })
}

fn parse_channel(value: Option<&&str>) -> Result<Channel, MidiShapeError> {
    Channel::new(parse_u8(
        value.copied(),
        MidiShapeError::InvalidChannelMessage,
    )?)
    .map_err(|_| MidiShapeError::InvalidChannelMessage)
}

fn parse_u7(value: Option<&&str>) -> Result<U7, MidiShapeError> {
    U7::try_from(
        value
            .copied()
            .ok_or(MidiShapeError::InvalidChannelMessage)?
            .parse::<u16>()
            .map_err(|_| MidiShapeError::InvalidChannelMessage)?,
    )
    .map_err(|_| MidiShapeError::InvalidChannelMessage)
}

fn parse_u14(value: Option<&&str>) -> Result<U14, MidiShapeError> {
    U14::try_from(
        value
            .copied()
            .ok_or(MidiShapeError::InvalidChannelMessage)?
            .parse::<u16>()
            .map_err(|_| MidiShapeError::InvalidChannelMessage)?,
    )
    .map_err(|_| MidiShapeError::InvalidChannelMessage)
}

fn parse_u8<T>(value: Option<T>, error: MidiShapeError) -> Result<u8, MidiShapeError>
where
    T: AsRef<str>,
{
    value
        .ok_or(error.clone())?
        .as_ref()
        .parse::<u8>()
        .map_err(|_| error)
}

fn parse_u32<T>(value: Option<T>, error: MidiShapeError) -> Result<u32, MidiShapeError>
where
    T: AsRef<str>,
{
    value
        .ok_or(error.clone())?
        .as_ref()
        .parse::<u32>()
        .map_err(|_| error)
}

fn parse_i8<T>(value: Option<T>, error: MidiShapeError) -> Result<i8, MidiShapeError>
where
    T: AsRef<str>,
{
    value
        .ok_or(error.clone())?
        .as_ref()
        .parse::<i8>()
        .map_err(|_| error)
}

fn parse_ratio(value: &str) -> Result<(i64, i64), MidiShapeError> {
    if let Some((num, den)) = value.split_once('/') {
        let numerator = num
            .parse::<i64>()
            .map_err(|_| MidiShapeError::InvalidTickTime)?;
        let denominator = den
            .parse::<i64>()
            .map_err(|_| MidiShapeError::InvalidTickTime)?;
        if denominator == 0 {
            return Err(MidiShapeError::InvalidTickTime);
        }
        Ok((numerator, denominator))
    } else {
        Ok((
            value
                .parse::<i64>()
                .map_err(|_| MidiShapeError::InvalidTickTime)?,
            1,
        ))
    }
}

fn encode_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn decode_bytes(value: &str) -> Result<Vec<u8>, MidiShapeError> {
    if !value.len().is_multiple_of(2) {
        return Err(MidiShapeError::InvalidRaw);
    }
    (0..value.len())
        .step_by(2)
        .map(|idx| {
            u8::from_str_radix(&value[idx..idx + 2], 16).map_err(|_| MidiShapeError::InvalidRaw)
        })
        .collect()
}

fn split_top_level(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let chars = value.char_indices().collect::<Vec<_>>();
    for (idx, ch) in &chars {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                if start < *idx {
                    parts.push(value[start..*idx].trim());
                }
                start = *idx + 1;
            }
            _ => {}
        }
    }
    if start < value.len() {
        parts.push(value[start..].trim());
    }
    parts.into_iter().filter(|part| !part.is_empty()).collect()
}
