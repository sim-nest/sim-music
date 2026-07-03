use super::*;

use sim_lib_midi_core::{
    Channel, ChannelMessage, MidiEvent, MidiPayload, TickTime, U7, synthetic_origin,
};

#[test]
fn midi_event_frame_round_trips_note_on() {
    let event = MidiEvent {
        time: TickTime::new(480, 480).unwrap(),
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel(0),
            key: U7(60),
            vel: U7(100),
        }),
    };
    let frame = MidiEventFrame::from_event(&event);
    assert_eq!(frame.to_event().unwrap().payload, event.payload);
}

#[test]
fn midi_event_frame_arrays_cross_boundary() {
    let frames = vec![
        MidiEventFrame {
            ticks: 0,
            tpq: 480,
            kind: MidiFrameKind::Meta,
            status: 0x2f,
            data: Vec::new(),
        },
        MidiEventFrame {
            ticks: 120,
            tpq: 480,
            kind: MidiFrameKind::Raw,
            status: 0xf4,
            data: vec![1, 2, 3],
        },
    ];
    let boundary = frame_array_boundary(&frames).unwrap();
    let decoded = decode_frame_array(boundary.bytes()).unwrap();
    assert_eq!(decoded, frames);
}
