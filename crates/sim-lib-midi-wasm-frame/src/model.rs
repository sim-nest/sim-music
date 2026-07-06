use thiserror::Error;

use sim_lib_midi_core::{
    ChannelMessage, MetaBucket, MetaEvent, MidiError, MidiEvent, MidiPayload, RawBytes, SysExEvent,
    TickTime, synthetic_origin, wire,
};
use sim_lib_midi_smf::{SmfError, SmfFile, SmfFormat, SmfTrack, read_smf, write_smf};
use sim_wasm_abi::Frame;

const HEADER_BYTES: usize = 18;

/// Errors raised while encoding or decoding MIDI binary frames.
#[derive(Debug, Error)]
pub enum MidiWasmError {
    /// An underlying Standard MIDI File read or write failed.
    #[error(transparent)]
    Smf(#[from] SmfError),
    /// A frame was truncated or otherwise malformed.
    #[error("invalid midi event frame")]
    InvalidFrame,
    /// The frame kind byte did not match a known [`MidiFrameKind`].
    #[error("unknown midi frame kind {0}")]
    UnknownKind(u8),
    /// A status byte was invalid for its frame kind.
    #[error("invalid midi status byte 0x{0:02x}")]
    InvalidStatus(u8),
}

/// The payload family carried by a [`MidiEventFrame`], encoded as the kind byte.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MidiFrameKind {
    /// A channel-voice message.
    Channel = 0,
    /// A meta event.
    Meta = 1,
    /// A system-exclusive event.
    SysEx = 2,
    /// Uninterpreted raw bytes.
    Raw = 3,
}

/// A frame-safe, fixed-header MIDI event ready for ABI transport.
///
/// The 18-byte header carries timing, kind, and status; variable-length data
/// follows. Use [`from_event`](Self::from_event)/[`to_event`](Self::to_event)
/// to convert to and from the [`MidiEvent`] model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiEventFrame {
    /// Event tick offset.
    pub ticks: i64,
    /// Resolution in ticks per quarter note.
    pub tpq: u32,
    /// Which payload family this frame carries.
    pub kind: MidiFrameKind,
    /// Status byte for the payload.
    pub status: u8,
    /// Payload data bytes.
    pub data: Vec<u8>,
}

/// A flattened, display-oriented row describing one MIDI event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiEventRow {
    /// Source track index.
    pub track: usize,
    /// Event tick offset.
    pub ticks: i64,
    /// Resolution in ticks per quarter note.
    pub tpq: u32,
    /// Payload family name (`channel`, `meta`, `sysex`, or `raw`).
    pub payload: String,
    /// Status byte.
    pub status: u8,
    /// Hex-encoded data bytes.
    pub data_hex: String,
}

impl MidiFrameKind {
    fn parse(value: u8) -> Result<Self, MidiWasmError> {
        match value {
            0 => Ok(Self::Channel),
            1 => Ok(Self::Meta),
            2 => Ok(Self::SysEx),
            3 => Ok(Self::Raw),
            other => Err(MidiWasmError::UnknownKind(other)),
        }
    }
}

impl MidiEventFrame {
    /// Builds a frame from a [`MidiEvent`], selecting the kind from its payload.
    pub fn from_event(event: &MidiEvent) -> Self {
        match &event.payload {
            MidiPayload::Channel(message) => encode_channel(event.time, message),
            MidiPayload::Meta(meta) => encode_meta(event.time, meta),
            MidiPayload::SysEx(sys_ex) => encode_sysex(event.time, sys_ex),
            MidiPayload::Raw(raw) => Self {
                ticks: event.time.ticks,
                tpq: event.time.tpq,
                kind: MidiFrameKind::Raw,
                status: raw.status,
                data: raw.data.clone(),
            },
        }
    }

    /// Reconstructs a [`MidiEvent`] from this frame, tagging it with a synthetic
    /// origin. Fails with [`MidiWasmError`] on invalid timing or payload.
    pub fn to_event(&self) -> Result<MidiEvent, MidiWasmError> {
        let time = TickTime::new(self.ticks, self.tpq).map_err(|_| MidiWasmError::InvalidFrame)?;
        let payload = match self.kind {
            MidiFrameKind::Channel => {
                MidiPayload::Channel(decode_channel(self.status, &self.data)?)
            }
            MidiFrameKind::Meta => MidiPayload::Meta(decode_meta(self.status, &self.data)?),
            MidiFrameKind::SysEx => MidiPayload::SysEx(decode_sysex(self.status, &self.data)?),
            MidiFrameKind::Raw => MidiPayload::Raw(RawBytes {
                status: self.status,
                data: self.data.clone(),
            }),
        };
        Ok(MidiEvent {
            time,
            origin: synthetic_origin(),
            payload,
        })
    }

    /// Appends this frame's little-endian byte encoding to `out`.
    ///
    /// Fails with [`MidiWasmError::InvalidFrame`] if the data exceeds the
    /// 16-bit length field.
    pub fn encode_into(&self, out: &mut Vec<u8>) -> Result<(), MidiWasmError> {
        let data_len = u16::try_from(self.data.len()).map_err(|_| MidiWasmError::InvalidFrame)?;
        out.extend_from_slice(&self.ticks.to_le_bytes());
        out.extend_from_slice(&self.tpq.to_le_bytes());
        out.push(self.kind as u8);
        out.push(self.status);
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend_from_slice(&[0_u8; 2]);
        out.extend_from_slice(&self.data);
        Ok(())
    }
}

/// Encodes a slice of frames into a single contiguous byte buffer.
pub fn encode_frame_array(frames: &[MidiEventFrame]) -> Result<Vec<u8>, MidiWasmError> {
    let mut out = Vec::new();
    for frame in frames {
        frame.encode_into(&mut out)?;
    }
    Ok(out)
}

/// Decodes a contiguous byte buffer back into a vector of frames.
pub fn decode_frame_array(bytes: &[u8]) -> Result<Vec<MidiEventFrame>, MidiWasmError> {
    let mut cursor = 0;
    let mut frames = Vec::new();
    while cursor < bytes.len() {
        if bytes.len() - cursor < HEADER_BYTES {
            return Err(MidiWasmError::InvalidFrame);
        }
        let ticks = i64::from_le_bytes(
            bytes[cursor..cursor + 8]
                .try_into()
                .map_err(|_| MidiWasmError::InvalidFrame)?,
        );
        let tpq = u32::from_le_bytes(
            bytes[cursor + 8..cursor + 12]
                .try_into()
                .map_err(|_| MidiWasmError::InvalidFrame)?,
        );
        let kind = MidiFrameKind::parse(bytes[cursor + 12])?;
        let status = bytes[cursor + 13];
        let data_len = u16::from_le_bytes(
            bytes[cursor + 14..cursor + 16]
                .try_into()
                .map_err(|_| MidiWasmError::InvalidFrame)?,
        ) as usize;
        let total = HEADER_BYTES + data_len;
        if bytes.len() - cursor < total {
            return Err(MidiWasmError::InvalidFrame);
        }
        let data = bytes[cursor + HEADER_BYTES..cursor + total].to_vec();
        frames.push(MidiEventFrame {
            ticks,
            tpq,
            kind,
            status,
            data,
        });
        cursor += total;
    }
    Ok(frames)
}

/// Encodes frames and wraps the bytes in an ABI [`Frame`] boundary value.
pub fn frame_array_boundary(frames: &[MidiEventFrame]) -> Result<Frame, MidiWasmError> {
    Ok(Frame::new(encode_frame_array(frames)?))
}

/// Flattens every track of an [`SmfFile`] into a time-ordered frame vector.
pub fn smf_to_event_frames(file: &SmfFile) -> Vec<MidiEventFrame> {
    file.merged_events()
        .into_iter()
        .map(|tracked| MidiEventFrame::from_event(&tracked.event))
        .collect()
}

/// Rebuilds a single-track [`SmfFile`] of the given `format` from `frames`,
/// taking the resolution from the first frame (defaulting to 480).
pub fn frame_array_to_smf(
    frames: &[MidiEventFrame],
    format: SmfFormat,
) -> Result<SmfFile, MidiWasmError> {
    let events = frames
        .iter()
        .map(MidiEventFrame::to_event)
        .collect::<Result<Vec<_>, _>>()?;
    let tpq = frames.first().map(|frame| frame.tpq).unwrap_or(480);
    let mut file = SmfFile {
        format,
        tpq,
        tracks: vec![SmfTrack { events }],
    };
    file.canonicalize();
    Ok(file)
}

/// Reads SMF bytes, converts through the frame model, and re-serialises them,
/// exercising the full round-trip.
pub fn roundtrip_smf_bytes(bytes: &[u8]) -> Result<Vec<u8>, MidiWasmError> {
    let file = read_smf(bytes)?;
    let frames = smf_to_event_frames(&file);
    let roundtrip = frame_array_to_smf(&frames, SmfFormat::SingleTrack)?;
    Ok(write_smf(&roundtrip)?)
}

/// Reads SMF bytes and returns one display-oriented [`MidiEventRow`] per event.
pub fn midi_event_table(bytes: &[u8]) -> Result<Vec<MidiEventRow>, MidiWasmError> {
    let file = read_smf(bytes)?;
    Ok(file
        .merged_events()
        .into_iter()
        .map(|tracked| {
            let frame = MidiEventFrame::from_event(&tracked.event);
            MidiEventRow {
                track: tracked.last_track,
                ticks: tracked.event.time.ticks,
                tpq: tracked.event.time.tpq,
                payload: payload_name(&tracked.event.payload).to_owned(),
                status: frame.status,
                data_hex: hex(&frame.data),
            }
        })
        .collect())
}

fn encode_channel(time: TickTime, message: &ChannelMessage) -> MidiEventFrame {
    let (status, data) = sim_lib_midi_core::wire::encode_channel(message);
    MidiEventFrame {
        ticks: time.ticks,
        tpq: time.tpq,
        kind: MidiFrameKind::Channel,
        status,
        data,
    }
}

fn decode_channel(status: u8, data: &[u8]) -> Result<ChannelMessage, MidiWasmError> {
    wire::decode_channel(status, data).map_err(|error| match error {
        MidiError::TruncatedChannel => MidiWasmError::InvalidFrame,
        _ => MidiWasmError::InvalidStatus(status),
    })
}

fn encode_meta(time: TickTime, meta: &MetaEvent) -> MidiEventFrame {
    let (status, data) = match meta {
        MetaEvent::EndOfTrack => (0x2f, Vec::new()),
        MetaEvent::Tempo { us_per_quarter } => {
            let bytes = us_per_quarter.to_be_bytes();
            (0x51, bytes[1..].to_vec())
        }
        MetaEvent::TimeSig {
            num,
            den_pow2,
            clocks_per_click,
            thirty_seconds_per_quarter,
        } => (
            0x58,
            vec![
                *num,
                *den_pow2,
                *clocks_per_click,
                *thirty_seconds_per_quarter,
            ],
        ),
        MetaEvent::KeySig {
            sharps_flats,
            minor,
        } => (0x59, vec![*sharps_flats as u8, u8::from(*minor)]),
        MetaEvent::Other(bucket) => (bucket.type_byte, bucket.data.clone()),
    };
    MidiEventFrame {
        ticks: time.ticks,
        tpq: time.tpq,
        kind: MidiFrameKind::Meta,
        status,
        data,
    }
}

fn decode_meta(status: u8, data: &[u8]) -> Result<MetaEvent, MidiWasmError> {
    match status {
        0x2f if data.is_empty() => Ok(MetaEvent::EndOfTrack),
        0x51 if data.len() == 3 => Ok(MetaEvent::Tempo {
            us_per_quarter: u32::from_be_bytes([0, data[0], data[1], data[2]]),
        }),
        0x58 if data.len() == 4 => Ok(MetaEvent::TimeSig {
            num: data[0],
            den_pow2: data[1],
            clocks_per_click: data[2],
            thirty_seconds_per_quarter: data[3],
        }),
        0x59 if data.len() == 2 => Ok(MetaEvent::KeySig {
            sharps_flats: data[0] as i8,
            minor: data[1] != 0,
        }),
        type_byte => Ok(MetaEvent::Other(MetaBucket {
            type_byte,
            data: data.to_vec(),
        })),
    }
}

fn encode_sysex(time: TickTime, event: &SysExEvent) -> MidiEventFrame {
    let (status, data) = match event {
        SysExEvent::F0 { data } => (0xf0, data.clone()),
        SysExEvent::F7 { data } => (0xf7, data.clone()),
    };
    MidiEventFrame {
        ticks: time.ticks,
        tpq: time.tpq,
        kind: MidiFrameKind::SysEx,
        status,
        data,
    }
}

fn decode_sysex(status: u8, data: &[u8]) -> Result<SysExEvent, MidiWasmError> {
    match status {
        0xf0 => Ok(SysExEvent::F0 {
            data: data.to_vec(),
        }),
        0xf7 => Ok(SysExEvent::F7 {
            data: data.to_vec(),
        }),
        _ => Err(MidiWasmError::InvalidStatus(status)),
    }
}

fn payload_name(payload: &MidiPayload) -> &'static str {
    match payload {
        MidiPayload::Channel(_) => "channel",
        MidiPayload::Meta(_) => "meta",
        MidiPayload::SysEx(_) => "sysex",
        MidiPayload::Raw(_) => "raw",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}
