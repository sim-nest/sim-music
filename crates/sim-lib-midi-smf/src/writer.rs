#![forbid(unsafe_code)]

use sim_lib_midi_core::{MetaEvent, MidiEvent, MidiPayload, RawBytes, SysExEvent};

use crate::{
    SmfError, SmfFile, SmfFormat, SmfTrack, SmfWriteOptions, canonicalize_track, encode_vlq,
};

/// Largest value that SMF permits in a four-byte variable-length quantity.
pub(crate) const MAX_SMF_VLQ: u32 = 0x0fff_ffff;

/// Serialises `file` to SMF bytes using default options.
pub fn write_smf(file: &SmfFile) -> Result<Vec<u8>, SmfError> {
    write_smf_with_options(file, SmfWriteOptions::default())
}

/// Serialises `file` to SMF bytes under the given [`SmfWriteOptions`].
///
/// Each track is canonicalised before writing. Fails with [`SmfError`] when the
/// format and track count disagree or an event time cannot be expressed at the
/// file resolution.
pub fn write_smf_with_options(
    file: &SmfFile,
    options: SmfWriteOptions,
) -> Result<Vec<u8>, SmfError> {
    if matches!(file.format, SmfFormat::SingleTrack) && file.tracks.len() != 1 {
        return Err(SmfError::FormatTrackMismatch);
    }
    let track_count = u16::try_from(file.tracks.len())
        .map_err(|_| SmfError::TrackCountOutOfRange(file.tracks.len()))?;
    let tpq = checked_tpq(file.tpq)?;
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    let format = match file.format {
        SmfFormat::SingleTrack => 0u16,
        SmfFormat::Simultaneous => 1u16,
        SmfFormat::Independent => 2u16,
    };
    out.extend_from_slice(&format.to_be_bytes());
    out.extend_from_slice(&track_count.to_be_bytes());
    out.extend_from_slice(&tpq.to_be_bytes());
    for track in &file.tracks {
        let body = write_track(track, file.tpq, options)?;
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&checked_chunk_len(body.len())?.to_be_bytes());
        out.extend_from_slice(&body);
    }
    Ok(out)
}

fn write_track(
    track: &SmfTrack,
    file_tpq: u32,
    options: SmfWriteOptions,
) -> Result<Vec<u8>, SmfError> {
    let mut track = track.clone();
    canonicalize_track(&mut track, file_tpq);
    let mut body = Vec::new();
    let mut last_tick = 0i64;
    let mut last_status: Option<u8> = None;
    for event in &track.events {
        let time = if event.time.tpq == file_tpq {
            event.time
        } else {
            event
                .time
                .rebase(file_tpq)
                .map_err(|_| SmfError::InexactEventTime)?
        };
        let delta = time.ticks - last_tick;
        if delta < 0 {
            return Err(SmfError::NegativeDelta);
        }
        body.extend_from_slice(&encode_vlq(checked_delta(delta)?));
        last_tick = time.ticks;
        let status = write_payload(&mut body, event, options, last_status)?;
        last_status = status;
    }
    Ok(body)
}

fn write_payload(
    out: &mut Vec<u8>,
    event: &MidiEvent,
    options: SmfWriteOptions,
    last_status: Option<u8>,
) -> Result<Option<u8>, SmfError> {
    match &event.payload {
        MidiPayload::Channel(message) => {
            let (status, data) = encode_channel(message);
            if !(options.running_status && last_status == Some(status)) {
                out.push(status);
            }
            out.extend_from_slice(&data);
            Ok(Some(status))
        }
        MidiPayload::Meta(event) => {
            let (type_byte, data) = encode_meta(event);
            out.push(0xff);
            out.push(type_byte);
            out.extend_from_slice(&encode_vlq(checked_payload_len(data.len())?));
            out.extend_from_slice(&data);
            Ok(None)
        }
        MidiPayload::SysEx(event) => {
            let (status, data) = match event {
                SysExEvent::F0 { data } => (0xf0, data.as_slice()),
                SysExEvent::F7 { data } => (0xf7, data.as_slice()),
            };
            out.push(status);
            out.extend_from_slice(&encode_vlq(checked_payload_len(data.len())?));
            out.extend_from_slice(data);
            Ok(None)
        }
        MidiPayload::Raw(raw) => {
            write_raw(out, raw);
            Ok(None)
        }
    }
}

use sim_lib_midi_core::wire::encode_channel;

fn encode_meta(event: &MetaEvent) -> (u8, Vec<u8>) {
    match event {
        MetaEvent::EndOfTrack => (0x2f, Vec::new()),
        MetaEvent::Tempo { us_per_quarter } => (
            0x51,
            vec![
                masked_u8(us_per_quarter >> 16),
                masked_u8(us_per_quarter >> 8),
                masked_u8(*us_per_quarter),
            ],
        ),
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
        } => (
            0x59,
            vec![
                u8::from_ne_bytes(sharps_flats.to_ne_bytes()),
                u8::from(*minor),
            ],
        ),
        MetaEvent::Other(bucket) => (bucket.type_byte, bucket.data.clone()),
    }
}

/// Checks that a ticks-per-quarter value fits the SMF metrical division field.
pub(crate) fn checked_tpq(tpq: u32) -> Result<u16, SmfError> {
    if tpq == 0 || tpq >= 0x8000 {
        return Err(SmfError::TpqOutOfRange(tpq));
    }
    u16::try_from(tpq).map_err(|_| SmfError::TpqOutOfRange(tpq))
}

/// Checks that a delta fits the SMF four-byte VLQ range.
pub(crate) fn checked_delta(delta: i64) -> Result<u32, SmfError> {
    if delta > i64::from(MAX_SMF_VLQ) {
        return Err(SmfError::DeltaOutOfRange(delta));
    }
    u32::try_from(delta).map_err(|_| SmfError::NegativeDelta)
}

/// Checks that a track chunk body fits the SMF chunk length field.
pub(crate) fn checked_chunk_len(len: usize) -> Result<u32, SmfError> {
    u32::try_from(len).map_err(|_| SmfError::ChunkTooLarge(len))
}

/// Checks that a meta or SysEx payload length fits the SMF four-byte VLQ range.
pub(crate) fn checked_payload_len(len: usize) -> Result<u32, SmfError> {
    let max = usize::try_from(MAX_SMF_VLQ).expect("SMF VLQ maximum fits usize");
    if len > max {
        return Err(SmfError::PayloadTooLarge(len));
    }
    u32::try_from(len).map_err(|_| SmfError::PayloadTooLarge(len))
}

fn masked_u8(value: u32) -> u8 {
    u8::try_from(value & 0xff).expect("masked MIDI byte fits u8")
}

fn write_raw(out: &mut Vec<u8>, raw: &RawBytes) {
    out.push(raw.status);
    out.extend_from_slice(&raw.data);
}
