use sim_kernel::{Expr, Symbol};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaEvent, MidiEvent, MidiPayload, TickTime, U7, synthetic_origin,
};
use sim_lib_stream_core::{
    BufferPolicy, MidiPacket, PcmPacket, StreamDirection, StreamItem, StreamMedia, StreamMetadata,
    StreamPacket, StreamValue,
};
use sim_lib_stream_midi::{midi_event_to_packet_event, midi_packet_to_events};

use crate::{
    StreamBridgeLiftMidiOptions, StreamBridgeRenderOptions, lift_pcm_stream_to_midi,
    render_midi_stream_to_pcm,
};

#[test]
fn note_on_off_renders_non_empty_pcm() {
    let output = render_midi_stream_to_pcm(
        &midi_stream(vec![note_on(0, 69), note_off(480, 69)]),
        StreamBridgeRenderOptions::default(),
    )
    .unwrap();

    let samples = pcm_samples(&output.stream);

    assert!(samples.iter().any(|sample| *sample != 0));
}

#[test]
fn tempo_change_moves_frame_placement_deterministically() {
    let fast = first_nonzero_frame(&rendered_samples(500_000));
    let slow = first_nonzero_frame(&rendered_samples(1_000_000));
    let slow_again = first_nonzero_frame(&rendered_samples(1_000_000));

    assert_eq!(slow, slow_again);
    assert!(slow > fast + 20_000, "fast={fast} slow={slow}");
}

#[test]
fn sine_input_lifts_to_expected_midi_note_range() {
    let output = lift_pcm_stream_to_midi(
        &pcm_stream(&sine(440.0, 48_000, 0.25, 1.0)),
        StreamBridgeLiftMidiOptions::default(),
    )
    .unwrap();

    let notes = midi_note_ons(&output.stream);

    assert!(
        notes.iter().any(|note| (68..=70).contains(note)),
        "{notes:?}"
    );
}

#[test]
fn silence_lifts_to_no_notes() {
    let output = lift_pcm_stream_to_midi(
        &pcm_stream(&vec![0; 48_000 / 4]),
        StreamBridgeLiftMidiOptions::default(),
    )
    .unwrap();

    assert!(midi_note_ons(&output.stream).is_empty());
}

#[test]
fn low_confidence_emits_diagnostics() {
    let output = lift_pcm_stream_to_midi(
        &pcm_stream(&sine(440.0, 48_000, 0.25, 1.0)),
        StreamBridgeLiftMidiOptions {
            min_confidence: 0.99,
            ..StreamBridgeLiftMidiOptions::default()
        },
    )
    .unwrap();

    assert!(
        diagnostic_messages(&output.stream)
            .iter()
            .any(|message| message.contains("no note candidates"))
    );
}

#[test]
fn bridge_render_options_reject_invalid_public_fields() {
    assert!(StreamBridgeRenderOptions::new(0, 2, 512).is_err());
    assert!(StreamBridgeRenderOptions::new(48_000, 3, 512).is_err());
    assert!(StreamBridgeRenderOptions::new(48_000, 2, 0).is_err());
}

#[test]
fn bridge_lift_options_reject_invalid_public_fields() {
    let defaults = StreamBridgeLiftMidiOptions::default();

    assert!(
        StreamBridgeLiftMidiOptions {
            sample_rate: 0,
            ..defaults.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        StreamBridgeLiftMidiOptions {
            tpq: 0,
            ..defaults.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        StreamBridgeLiftMidiOptions {
            min_confidence: f64::NAN,
            ..defaults.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        StreamBridgeLiftMidiOptions {
            window_size: 0,
            ..defaults.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        StreamBridgeLiftMidiOptions {
            hop_size: 0,
            ..defaults.clone()
        }
        .validate()
        .is_err()
    );
    assert!(
        StreamBridgeLiftMidiOptions {
            max_events_per_packet: 0,
            ..defaults
        }
        .validate()
        .is_err()
    );
}

#[test]
fn bridge_media_adapters_reject_data_packets() {
    let render_err =
        match render_midi_stream_to_pcm(&data_stream(), StreamBridgeRenderOptions::default()) {
            Ok(_) => panic!("MIDI render unexpectedly accepted data packets"),
            Err(error) => error,
        };
    assert!(format!("{render_err}").contains("expects MIDI stream packets"));

    let lift_err =
        match lift_pcm_stream_to_midi(&data_stream(), StreamBridgeLiftMidiOptions::default()) {
            Ok(_) => panic!("PCM lift unexpectedly accepted data packets"),
            Err(error) => error,
        };
    assert!(format!("{lift_err}").contains("expects PCM stream packets"));
}

#[test]
fn citizen_bridge_option_descriptors_round_trip() {
    let mut cx = sim_kernel::Cx::new(
        std::sync::Arc::new(sim_kernel::NoopEvalPolicy),
        std::sync::Arc::new(sim_kernel::DefaultFactory),
    );
    cx.load_lib(&sim_citizen::CitizenLib::all()).unwrap();

    sim_citizen::check_default_fixture::<StreamBridgeRenderOptions>(&mut cx).unwrap();
    sim_citizen::check_default_fixture::<StreamBridgeLiftMidiOptions>(&mut cx).unwrap();
}

fn rendered_samples(us_per_quarter: u32) -> Vec<i16> {
    let output = render_midi_stream_to_pcm(
        &midi_stream(vec![
            tempo(0, us_per_quarter),
            note_on(480, 69),
            note_off(960, 69),
        ]),
        StreamBridgeRenderOptions::default(),
    )
    .unwrap();
    pcm_samples(&output.stream)
}

fn midi_stream(events: Vec<MidiEvent>) -> StreamValue {
    let packet = MidiPacket::new(
        events
            .iter()
            .map(midi_event_to_packet_event)
            .collect::<sim_kernel::Result<Vec<_>>>()
            .unwrap(),
    )
    .unwrap();
    StreamValue::pull(
        metadata(StreamMedia::Midi, SymbolSuffix::Midi),
        vec![StreamItem::new(StreamPacket::Midi(packet))],
    )
}

fn pcm_stream(samples: &[i16]) -> StreamValue {
    let packet = PcmPacket::i16(1, samples.len(), samples.to_vec()).unwrap();
    StreamValue::pull(
        metadata(StreamMedia::Pcm, SymbolSuffix::Pcm),
        vec![StreamItem::new(StreamPacket::Pcm(packet))],
    )
}

fn data_stream() -> StreamValue {
    StreamValue::pull(
        metadata(StreamMedia::Data, SymbolSuffix::Data),
        vec![StreamItem::new(StreamPacket::data(
            Symbol::qualified("stream/data", "expr"),
            Expr::String("payload".to_owned()),
        ))],
    )
}

fn pcm_samples(stream: &StreamValue) -> Vec<i16> {
    let mut out = Vec::new();
    while let Some(item) = stream.next_packet().unwrap() {
        if let StreamPacket::Pcm(packet) = item.packet() {
            out.extend_from_slice(packet.samples_i16());
        }
    }
    out
}

fn midi_note_ons(stream: &StreamValue) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(item) = stream.next_packet().unwrap() {
        let StreamPacket::Midi(packet) = item.packet() else {
            continue;
        };
        for event in midi_packet_to_events(packet).unwrap() {
            if let MidiPayload::Channel(ChannelMessage::NoteOn { key, vel, .. }) = event.payload
                && vel.0 > 0
            {
                out.push(key.0);
            }
        }
    }
    out
}

fn diagnostic_messages(stream: &StreamValue) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(item) = stream.next_packet().unwrap() {
        if let StreamPacket::Diagnostic(packet) = item.packet() {
            out.push(packet.message().to_owned());
        }
    }
    out
}

fn first_nonzero_frame(samples: &[i16]) -> usize {
    samples
        .chunks(2)
        .position(|frame| frame.iter().any(|sample| *sample != 0))
        .unwrap()
}

fn sine(hz: f64, sample_rate: u32, seconds: f64, gain: f64) -> Vec<i16> {
    let samples = (seconds * f64::from(sample_rate)).round() as usize;
    (0..samples)
        .map(|index| {
            let t = index as f64 / f64::from(sample_rate);
            ((std::f64::consts::TAU * hz * t).sin() * gain * f64::from(i16::MAX)).round() as i16
        })
        .collect()
}

fn note_on(ticks: i64, key: u8) -> MidiEvent {
    event(
        ticks,
        MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel(0),
            key: U7(key),
            vel: U7(100),
        }),
    )
}

fn note_off(ticks: i64, key: u8) -> MidiEvent {
    event(
        ticks,
        MidiPayload::Channel(ChannelMessage::NoteOff {
            ch: Channel(0),
            key: U7(key),
            vel: U7(0),
        }),
    )
}

fn tempo(ticks: i64, us_per_quarter: u32) -> MidiEvent {
    event(
        ticks,
        MidiPayload::Meta(MetaEvent::Tempo { us_per_quarter }),
    )
}

fn event(ticks: i64, payload: MidiPayload) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, 480).unwrap(),
        origin: synthetic_origin(),
        payload,
    }
}

enum SymbolSuffix {
    Midi,
    Pcm,
    Data,
}

fn metadata(media: StreamMedia, suffix: SymbolSuffix) -> StreamMetadata {
    let suffix = match suffix {
        SymbolSuffix::Midi => "test-midi",
        SymbolSuffix::Pcm => "test-pcm",
        SymbolSuffix::Data => "test-data",
    };
    StreamMetadata::new(
        sim_kernel::Symbol::qualified("stream/bridge", suffix),
        media,
        StreamDirection::Source,
        sim_kernel::Symbol::qualified("stream/clock", "test"),
        BufferPolicy::bounded(8).unwrap(),
    )
}
