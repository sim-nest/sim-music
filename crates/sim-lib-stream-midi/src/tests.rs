use std::convert::TryFrom;

use sim_kernel::Symbol;
use sim_lib_midi_core::{
    Channel, ChannelMessage, MemoryMidiSink, MemoryMidiSource, MemoryTrackedMidiSource, MidiEvent,
    MidiPayload, RawBytes, SysExEvent, TickTime, TrackedMidiEvent, U7, synthetic_origin,
};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, StreamDirection, StreamItem, StreamMedia, StreamMetadata,
    StreamPacket, TransportProfile,
};

use crate::{
    midi_packet_to_events, midi_packet_to_lan_control_envelope, midi_source_to_stream,
    midi_stream_to_sink, packetize_midi_source, packetize_tracked_midi_source,
    write_midi_packets_to_sink,
};

#[test]
fn packetization_preserves_order_sysex_and_raw_bytes() {
    let events = vec![
        note_on(0, 60),
        midi_event(
            12,
            480,
            MidiPayload::SysEx(SysExEvent::F0 {
                data: vec![0x7d, 0x10, 0x11],
            }),
        ),
        midi_event(
            24,
            480,
            MidiPayload::Raw(RawBytes {
                status: 0xf5,
                data: vec![0x01, 0x02],
            }),
        ),
    ];
    let mut source = MemoryMidiSource::new(480, events.clone());

    let packets = packetize_midi_source(&mut source, 8).unwrap();

    assert_eq!(packets.len(), 1);
    let packet_events = packets[0].events();
    assert_eq!(packet_events[0].bytes(), &[0x90, 60, 100]);
    assert_eq!(packet_events[1].bytes(), &[0xf0, 0x7d, 0x10, 0x11]);
    assert_eq!(packet_events[2].bytes(), &[0xf5, 0x01, 0x02]);
    assert_eq!(midi_packet_to_events(&packets[0]).unwrap(), events);
}

#[test]
fn max_events_batching_splits_as_expected() {
    let events = (0..5).map(|index| note_on(index * 10, 60)).collect();
    let mut source = MemoryMidiSource::new(480, events);

    let packets = packetize_midi_source(&mut source, 2).unwrap();

    let lengths = packets
        .iter()
        .map(|packet| packet.events().len())
        .collect::<Vec<_>>();
    assert_eq!(lengths, vec![2, 2, 1]);
}

#[test]
fn tpq_never_mixed_in_one_packet() {
    let events = vec![
        note_on(0, 60),
        midi_event(12, 960, note_payload(61)),
        midi_event(24, 480, note_payload(62)),
    ];
    let mut source = MemoryMidiSource::new(480, events);

    let packets = packetize_midi_source(&mut source, 8).unwrap();

    assert_eq!(packets.len(), 3);
    for packet in packets {
        let tpq = packet.events()[0].tpq();
        assert!(packet.events().iter().all(|event| event.tpq() == tpq));
    }
}

#[test]
fn tracked_packetization_preserves_tracks() {
    let tracked = vec![
        tracked_event(0, note_on(0, 60)),
        tracked_event(2, note_on(12, 61)),
        tracked_event(1, note_on(24, 62)),
    ];
    let mut source = MemoryTrackedMidiSource::new(480, tracked);

    let packets = packetize_tracked_midi_source(&mut source, 2).unwrap();

    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].tracks(), &[0, 2]);
    assert_eq!(packets[1].tracks(), &[1]);
}

#[test]
fn sink_reconstructs_original_event_sequence() {
    let events = vec![note_on(0, 60), note_on(12, 61), note_on(24, 62)];
    let mut source = MemoryMidiSource::new(480, events.clone());
    let packets = packetize_midi_source(&mut source, 8).unwrap();
    let mut sink = MemoryMidiSink::new(480);

    let count = write_midi_packets_to_sink(&packets, &mut sink).unwrap();

    assert_eq!(count, 3);
    assert_eq!(sink.events(), events.as_slice());
}

#[test]
fn midi_packets_form_lan_control_stream_envelopes() {
    let mut source = MemoryMidiSource::new(480, vec![note_on(0, 60)]);
    let mut packets = packetize_midi_source(&mut source, 8).unwrap();

    let envelope = midi_packet_to_lan_control_envelope(&metadata(), 4, packets.remove(0)).unwrap();

    assert_eq!(envelope.media(), StreamMedia::Midi);
    assert_eq!(envelope.sequence(), 4);
    assert_eq!(envelope.clock_domain(), ClockDomain::MidiTick);
    assert_eq!(
        envelope.profile().name(),
        TransportProfile::lan_midi_control().name()
    );
    assert!(matches!(envelope.packet(), StreamPacket::Midi(_)));
}

#[test]
fn source_and_sink_spine_adapters_preserve_midi_events() {
    let events = vec![note_on(0, 60), note_on(12, 61)];
    let mut source = MemoryMidiSource::new(480, events.clone());
    let stream = midi_source_to_stream(&mut source, 8, metadata()).unwrap();
    let mut sink = MemoryMidiSink::new(480);

    assert_eq!(midi_stream_to_sink(&stream, &mut sink).unwrap(), 2);
    assert_eq!(sink.events(), events.as_slice());
    assert!(stream.is_done().unwrap());
}

#[test]
fn stream_spine_yields_midi_packets_then_nil() {
    let mut source = MemoryMidiSource::new(480, vec![note_on(0, 60), note_on(12, 61)]);
    let stream = midi_source_to_stream(&mut source, 1, metadata()).unwrap();

    assert_midi_packet(stream.next_packet().unwrap(), 1);
    assert_midi_packet(stream.next_packet().unwrap(), 1);
    assert!(stream.next_packet().unwrap().is_none());
}

fn metadata() -> StreamMetadata {
    StreamMetadata::new(
        Symbol::qualified("stream", "midi-memory"),
        StreamMedia::Midi,
        StreamDirection::Source,
        Symbol::qualified("clock", "midi"),
        BufferPolicy::bounded(4).unwrap(),
    )
}

fn note_on(ticks: i64, key: u8) -> MidiEvent {
    midi_event(ticks, 480, note_payload(key))
}

fn note_payload(key: u8) -> MidiPayload {
    MidiPayload::Channel(ChannelMessage::NoteOn {
        ch: Channel::new(0).unwrap(),
        key: U7::try_from(u16::from(key)).unwrap(),
        vel: U7::try_from(100).unwrap(),
    })
}

fn midi_event(ticks: i64, tpq: u32, payload: MidiPayload) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, tpq).unwrap(),
        origin: synthetic_origin(),
        payload,
    }
}

fn tracked_event(last_track: usize, event: MidiEvent) -> TrackedMidiEvent {
    TrackedMidiEvent { last_track, event }
}

fn assert_midi_packet(item: Option<StreamItem>, expected_len: usize) {
    let item = item.expect("expected a MIDI stream item");
    let StreamPacket::Midi(packet) = item.packet() else {
        panic!("expected a MIDI packet");
    };
    assert_eq!(packet.events().len(), expected_len);
}
