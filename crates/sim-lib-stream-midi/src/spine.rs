use std::fmt::Debug;

use sim_kernel::{Error, Result};
use sim_lib_midi_core::MidiSink;
use sim_lib_stream_core::{
    StreamDirection, StreamItem, StreamMedia, StreamMetadata, StreamPacket, StreamValue,
};

use crate::{
    packetize_midi_source,
    sink::{midi_packet_to_events, normalize_for_sink},
};

/// Packetizes a MIDI source into a pull-mode MIDI [`StreamValue`].
///
/// `metadata` must describe MIDI media and must not be sink-only; otherwise an
/// error is returned. Packets carry at most `max_events` events each.
pub fn midi_source_to_stream<S>(
    source: &mut S,
    max_events: usize,
    metadata: StreamMetadata,
) -> Result<StreamValue>
where
    S: sim_lib_midi_core::MidiSource,
    S::Err: Debug,
{
    ensure_source_metadata(&metadata)?;
    let items = packetize_midi_source(source, max_events)?
        .into_iter()
        .map(|packet| StreamItem::new(StreamPacket::Midi(packet)))
        .collect();
    Ok(StreamValue::pull(metadata, items))
}

/// Drains a MIDI stream into a sink, returning the number of events written.
///
/// Each event is quantized to the sink's ticks-per-quarter before being written,
/// and the sink is flushed at the end. Errors on a non-MIDI stream packet.
pub fn midi_stream_to_sink<S>(stream: &StreamValue, sink: &mut S) -> Result<usize>
where
    S: MidiSink,
    S::Err: Debug,
{
    let mut count = 0usize;
    while let Some(item) = stream.next_packet()? {
        let StreamPacket::Midi(packet) = item.packet() else {
            return Err(Error::Eval(
                "MIDI sink adapter received a non-MIDI stream packet".to_owned(),
            ));
        };
        for mut event in midi_packet_to_events(packet)? {
            normalize_for_sink(&mut event, sink.tpq());
            sink.write(&event)
                .map_err(|err| Error::Eval(format!("MIDI sink error: {err:?}")))?;
            count += 1;
        }
    }
    sink.flush()
        .map_err(|err| Error::Eval(format!("MIDI sink flush error: {err:?}")))?;
    Ok(count)
}

fn ensure_source_metadata(metadata: &StreamMetadata) -> Result<()> {
    if metadata.media() != StreamMedia::Midi {
        return Err(Error::Eval(
            "MIDI source stream metadata must use MIDI media".to_owned(),
        ));
    }
    if metadata.direction() == StreamDirection::Sink {
        return Err(Error::Eval(
            "MIDI source stream metadata must not be sink-only".to_owned(),
        ));
    }
    Ok(())
}
