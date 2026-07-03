use std::convert::{Infallible, TryFrom};
use std::sync::Arc;

use sim_kernel::{DefaultFactory, EagerPolicy, Symbol};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MemoryMidiSink, MidiEvent, MidiPayload, MidiSink, MidiSource,
    TickTime, TrackedMidiEvent, TrackedMidiSource, U7, pump, synthetic_origin,
};

use super::*;

fn note_on(ticks: i64, tpq: u32, key: u8) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, tpq).expect("tick time"),
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel::new(0).expect("channel"),
            key: U7::try_from(u16::from(key)).expect("key"),
            vel: U7::try_from(96).expect("velocity"),
        }),
    }
}

#[test]
fn ring_buffers_reject_zero_capacity() {
    assert!(matches!(
        RingMidiBuffer::new(480, 0),
        Err(LiveMidiError::ZeroCapacity)
    ));
    assert!(matches!(
        RingTrackedMidiBuffer::new(480, 0),
        Err(LiveMidiError::ZeroCapacity)
    ));
}

#[test]
fn ring_midi_buffer_is_fifo_until_full() {
    let mut ring = RingMidiBuffer::new(480, 3).expect("ring");
    let first = note_on(0, 480, 60);
    let second = note_on(120, 480, 62);
    ring.write(&first)
        .unwrap_or_else(|never: Infallible| match never {});
    ring.write(&second)
        .unwrap_or_else(|never: Infallible| match never {});
    assert_eq!(ring.len(), 2);
    assert_eq!(
        ring.next()
            .unwrap_or_else(|never: Infallible| match never {}),
        Some(first)
    );
    assert_eq!(
        ring.next()
            .unwrap_or_else(|never: Infallible| match never {}),
        Some(second)
    );
    assert!(ring.is_empty());
}

#[test]
fn ring_midi_buffer_drops_oldest_when_full() {
    let mut ring = RingMidiBuffer::new(480, 2).expect("ring");
    let first = note_on(0, 480, 60);
    let second = note_on(120, 480, 62);
    let third = note_on(240, 480, 64);
    ring.write(&first)
        .unwrap_or_else(|never: Infallible| match never {});
    ring.write(&second)
        .unwrap_or_else(|never: Infallible| match never {});
    ring.write(&third)
        .unwrap_or_else(|never: Infallible| match never {});
    assert_eq!(ring.dropped_events(), 1);
    assert_eq!(ring.snapshot(), vec![second, third]);
}

#[test]
fn tracked_ring_preserves_track_provenance() {
    let mut ring = RingTrackedMidiBuffer::new(480, 2).expect("ring");
    let first = TrackedMidiEvent {
        last_track: 4,
        event: note_on(0, 480, 60),
    };
    let second = TrackedMidiEvent {
        last_track: 7,
        event: note_on(120, 480, 62),
    };
    ring.push_tracked_event(first.clone());
    ring.push_tracked_event(second.clone());
    assert_eq!(ring.n_tracks(), 8);
    assert_eq!(
        ring.next_tracked()
            .unwrap_or_else(|never: Infallible| match never {}),
        Some(first)
    );
    assert_eq!(ring.last_track(), 4);
    assert_eq!(
        ring.next_tracked()
            .unwrap_or_else(|never: Infallible| match never {}),
        Some(second)
    );
    assert_eq!(ring.last_track(), 7);
}

#[test]
fn tracked_ring_drops_oldest_when_full() {
    let mut ring = RingTrackedMidiBuffer::new(480, 1).expect("ring");
    ring.push_tracked_event(TrackedMidiEvent {
        last_track: 1,
        event: note_on(0, 480, 60),
    });
    let retained = TrackedMidiEvent {
        last_track: 2,
        event: note_on(120, 480, 62),
    };
    ring.push_tracked_event(retained.clone());
    assert_eq!(ring.dropped_events(), 1);
    assert_eq!(ring.snapshot(), vec![retained]);
}

#[test]
fn pump_moves_events_into_live_ring_and_preserves_order() {
    let first = note_on(0, 480, 60);
    let second = note_on(90, 480, 62);
    let mut source = RingMidiBuffer::new(480, 4).expect("ring");
    source
        .write(&first)
        .unwrap_or_else(|never: Infallible| match never {});
    source
        .write(&second)
        .unwrap_or_else(|never: Infallible| match never {});
    let mut sink = MemoryMidiSink::new(480);
    assert_eq!(pump(&mut source, &mut sink), Ok(2));
    assert_eq!(sink.events(), &[first, second]);
}

#[test]
fn pump_quantizes_when_live_ring_tpq_differs() {
    let mut source = RingMidiBuffer::new(3, 2).expect("ring");
    source
        .write(&note_on(1, 3, 60))
        .unwrap_or_else(|never: Infallible| match never {});
    let mut sink = MemoryMidiSink::new(480);
    assert_eq!(pump(&mut source, &mut sink), Ok(1));
    assert_eq!(
        sink.events()[0].time,
        TickTime::new(160, 480).expect("tick")
    );
}

#[test]
fn install_midi_live_lib_registers_ring_runtime_exports() {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_midi_live_lib(&mut cx).expect("install");
    install_midi_live_lib(&mut cx).expect("install");
    assert!(
        cx.resolve_value(&Symbol::qualified("midi", "RingMidiBuffer"))
            .is_ok()
    );
    assert!(
        cx.resolve_value(&Symbol::qualified("midi", "MidiLiveRegistry"))
            .is_ok()
    );
}

#[test]
fn live_midi_session_callback_enqueue_only_feeds_source_ring() {
    let event = note_on(0, 480, 67);
    let mut session =
        LiveMidiSession::with_ring(480, 4, LiveMidiDirection::Duplex).expect("live session");

    session
        .enqueue_from_callback(&event)
        .unwrap_or_else(|never: Infallible| match never {});

    assert_eq!(session.direction(), LiveMidiDirection::Duplex);
    assert!(session.sink_mut().is_some());
    assert_eq!(
        session
            .source_mut()
            .next()
            .unwrap_or_else(|never: Infallible| match never {}),
        Some(event)
    );
    session.close().unwrap();
}
