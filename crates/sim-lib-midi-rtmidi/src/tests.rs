use crate::*;
use sim_kernel::Symbol;
use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaEvent, MidiPayload, MidiSink, MidiSource, RawBytes, SysExEvent, U7,
};

#[test]
fn modeled_backend_preserves_timestamp_order_and_payload_bytes() {
    let timing = RtmidiTiming::new(480, 500_000).unwrap();
    let mut source = RtmidiBackend::fake()
        .with_timing(timing)
        .with_input_events(
            &Symbol::new("rtmidi/fake-in"),
            vec![RtmidiEvent::new(250_000, vec![0x90, 60, 100])],
        )
        .unwrap()
        .open_midi_source(&Symbol::new("rtmidi/fake-in"))
        .unwrap();
    let event = source.next().unwrap().unwrap();
    assert_eq!(event.time.ticks, 240);
    assert_eq!(
        bytes_from_payload(&event.payload).unwrap(),
        vec![0x90, 60, 100]
    );
    let mut sink = RtmidiMidiSink::new(480).unwrap();
    sink.write(&event).unwrap();
    sink.flush().unwrap();
    assert_eq!(sink.events(), &[event]);
}

#[test]
fn portable_codec_is_byte_exact_and_rejects_non_wire_meta() {
    let note = MidiPayload::Channel(ChannelMessage::NoteOn {
        ch: Channel(0),
        key: U7(60),
        vel: U7(100),
    });
    assert_eq!(
        payload_from_bytes(&bytes_from_payload(&note).unwrap()).unwrap(),
        note
    );
    let raw = MidiPayload::Raw(RawBytes {
        status: 0xf2,
        data: vec![1, 2],
    });
    assert_eq!(bytes_from_payload(&raw).unwrap(), vec![0xf2, 1, 2]);
    let sysex = MidiPayload::SysEx(SysExEvent::F0 {
        data: vec![0x7e, 0x7f],
    });
    assert_eq!(
        payload_from_bytes(&bytes_from_payload(&sysex).unwrap()).unwrap(),
        sysex
    );
    assert!(bytes_from_payload(&MidiPayload::Meta(MetaEvent::EndOfTrack)).is_err());
}
