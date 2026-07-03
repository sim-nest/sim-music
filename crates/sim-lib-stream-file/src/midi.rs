use std::path::Path;

use sim_kernel::{Error, Result};
use sim_lib_midi_core::MemoryMidiSource;
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTrack, read_smf, write_smf};
use sim_lib_stream_core::{StreamMetadata, StreamValue};
use sim_lib_stream_midi::{midi_source_to_stream, midi_stream_to_sink};

use crate::effect_io::{read_file_with_effect, write_file_with_effect};

/// Reads a Standard MIDI File from `path` and opens it as a MIDI stream.
///
/// The read is gated by the filesystem read capability and recorded as a
/// KERNEL 6 filesystem effect. Events are packetized at most `max_events` per
/// packet.
pub fn read_smf_stream(
    cx: &mut sim_kernel::Cx,
    path: impl AsRef<Path>,
    max_events: usize,
    metadata: StreamMetadata,
) -> Result<StreamValue> {
    let bytes = read_file_with_effect(cx, path)?;
    smf_bytes_to_stream(&bytes, max_events, metadata)
}

/// Parses Standard MIDI File bytes and opens them as a MIDI stream.
pub fn smf_bytes_to_stream(
    bytes: &[u8],
    max_events: usize,
    metadata: StreamMetadata,
) -> Result<StreamValue> {
    let file = read_smf(bytes).map_err(|err| Error::Eval(format!("malformed SMF file: {err}")))?;
    smf_file_to_stream(&file, max_events, metadata)
}

/// Opens an already-parsed Standard MIDI File as a MIDI stream.
///
/// Merges every track into a single ordered timeline and packetizes it at most
/// `max_events` events per packet.
pub fn smf_file_to_stream(
    file: &SmfFile,
    max_events: usize,
    metadata: StreamMetadata,
) -> Result<StreamValue> {
    let events = file
        .merged_events()
        .into_iter()
        .map(|tracked| tracked.event)
        .collect();
    let mut source = MemoryMidiSource::new(file.tpq, events);
    midi_source_to_stream(&mut source, max_events, metadata)
}

/// Drains a MIDI stream and writes it to `path` as a Standard MIDI File.
///
/// The write is gated by the filesystem write capability and recorded as a
/// KERNEL 6 filesystem effect. Returns the number of events written.
pub fn write_smf_stream(
    cx: &mut sim_kernel::Cx,
    path: impl AsRef<Path>,
    stream: &StreamValue,
    tpq: u32,
) -> Result<usize> {
    let (file, count) = stream_to_smf_file(stream, tpq)?;
    let bytes = write_smf(&file).map_err(|err| Error::Eval(format!("cannot write SMF: {err}")))?;
    write_file_with_effect(cx, path, bytes)?;
    Ok(count)
}

/// Drains a MIDI stream and encodes it as Standard MIDI File bytes.
///
/// Returns the encoded bytes together with the number of events written.
pub fn stream_to_smf_bytes(stream: &StreamValue, tpq: u32) -> Result<(Vec<u8>, usize)> {
    let (file, count) = stream_to_smf_file(stream, tpq)?;
    let bytes = write_smf(&file).map_err(|err| Error::Eval(format!("cannot write SMF: {err}")))?;
    Ok((bytes, count))
}

/// Drains a MIDI stream into a single-track [`SmfFile`] with `tpq` resolution.
///
/// Returns the assembled file and the number of events written. Errors when
/// `tpq` exceeds the Standard MIDI File range.
pub fn stream_to_smf_file(stream: &StreamValue, tpq: u32) -> Result<(SmfFile, usize)> {
    if u16::try_from(tpq).is_err() {
        return Err(Error::Eval(format!(
            "SMF TPQ {tpq} exceeds the Standard MIDI File range"
        )));
    }
    let mut sink = sim_lib_midi_core::MemoryMidiSink::new(tpq);
    let count = midi_stream_to_sink(stream, &mut sink)?;
    let file = SmfFile {
        format: SmfFormat::SingleTrack,
        tpq,
        tracks: vec![SmfTrack {
            events: sink.events().to_vec(),
        }],
    };
    Ok((file, count))
}
