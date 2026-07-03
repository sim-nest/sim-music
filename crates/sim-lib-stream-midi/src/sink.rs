use std::fmt::Debug;

use sim_kernel::{Error, Result};
use sim_lib_midi_core::{MidiEvent, MidiSink, TickTime, synthetic_origin};
use sim_lib_stream_core::{MidiPacket, MidiPacketEvent};

use crate::codec::bytes_to_payload;

/// Decodes a MIDI packet back into a vector of MIDI core events.
pub fn midi_packet_to_events(packet: &MidiPacket) -> Result<Vec<MidiEvent>> {
    packet
        .events()
        .iter()
        .map(midi_packet_event_to_event)
        .collect()
}

/// Writes MIDI packets to a sink, returning the number of events written.
///
/// Each event is quantized to the sink's ticks-per-quarter before being written,
/// and the sink is flushed at the end.
pub fn write_midi_packets_to_sink<S>(packets: &[MidiPacket], sink: &mut S) -> Result<usize>
where
    S: MidiSink,
    S::Err: Debug,
{
    let mut count = 0usize;
    for packet in packets {
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

pub(crate) fn midi_packet_event_to_event(event: &MidiPacketEvent) -> Result<MidiEvent> {
    Ok(MidiEvent {
        time: TickTime::new(event.ticks(), u32::from(event.tpq()))
            .map_err(|err| Error::Eval(format!("invalid MIDI event time: {err}")))?,
        origin: synthetic_origin(),
        payload: bytes_to_payload(event.bytes())?,
    })
}

pub(crate) fn normalize_for_sink(event: &mut MidiEvent, sink_tpq: u32) {
    if event.time.tpq != sink_tpq {
        event.time = event.time.quantize(sink_tpq);
    }
}
