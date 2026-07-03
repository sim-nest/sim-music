use std::convert::{Infallible, TryFrom};
use std::sync::Arc;

use super::*;
use sim_kernel::{DefaultFactory, EagerPolicy, Expr, Symbol};

#[test]
fn tick_time_associativity_and_identity_hold_for_same_tpq() {
    let a = TickTime::new(120, 480).expect("tick time");
    let b = TickTime::new(240, 480).expect("tick time");
    let c = TickTime::new(-60, 480).expect("tick time");
    let zero = TickTime::new(0, 480).expect("tick time");
    assert_eq!((a + b) + c, a + (b + c));
    assert_eq!(a + zero, a);
    assert_eq!(zero + a, a);
}

#[test]
fn rebase_is_exact_when_divisible() {
    let time = TickTime::new(960, 480).expect("tick time");
    assert_eq!(
        time.rebase(960).expect("exact rebase"),
        TickTime::new(1920, 960).expect("tick time")
    );
}

#[test]
fn quantize_preserves_exact_values_and_never_panics() {
    let exact = TickTime::new(1, 2).expect("tick time");
    assert_eq!(
        exact.quantize(480),
        TickTime::new(240, 480).expect("tick time")
    );
    let inexact = TickTime::new(1, 3).expect("tick time");
    let quantized = inexact.quantize(480);
    assert_eq!(quantized.tpq, 480);
}

#[test]
fn u7_and_u14_validate_ranges() {
    assert!(U7::try_from(127).is_ok());
    assert!(U7::try_from(128).is_err());
    assert!(U14::try_from(16_383).is_ok());
    assert!(U14::try_from(16_384).is_err());
}

#[test]
fn channel_validates_range() {
    assert!(Channel::new(15).is_ok());
    assert!(Channel::new(16).is_err());
}

fn note_on_at(ticks: i64, key: u8, velocity: u8, channel: u8) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, 480).expect("tick"),
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel::new(channel).expect("channel"),
            key: U7::try_from(u16::from(key)).expect("key"),
            vel: U7::try_from(u16::from(velocity)).expect("velocity"),
        }),
    }
}

fn note_off_at(ticks: i64, key: u8, channel: u8) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, 480).expect("tick"),
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOff {
            ch: Channel::new(channel).expect("channel"),
            key: U7::try_from(u16::from(key)).expect("key"),
            vel: U7(0),
        }),
    }
}

fn rendered_note_ons(events: &[MidiEvent]) -> Vec<(i64, u8, u8, u8)> {
    events
        .iter()
        .filter_map(|event| match event.payload {
            MidiPayload::Channel(ChannelMessage::NoteOn { ch, key, vel }) if vel.0 > 0 => {
                Some((event.time.ticks, ch.0, key.0, vel.0))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn note_echo_repeats_polyphonic_input_with_velocity_cascade() {
    let input = vec![
        note_on_at(0, 60, 100, 0),
        note_on_at(0, 64, 96, 0),
        note_off_at(240, 60, 0),
        note_off_at(240, 64, 0),
    ];
    let mut config = NoteEchoConfig::new(TickTime::new(120, 480).expect("offset"));
    config.repeats = 4;
    config.feedback_count = 2;
    config.velocity_decay = 20;
    let player = NoteEchoPlayer::new(config);

    let render = player.render(&input);

    assert_eq!(
        rendered_note_ons(&render.events),
        vec![
            (0, 0, 60, 100),
            (0, 0, 64, 96),
            (120, 0, 60, 80),
            (120, 0, 64, 76),
            (240, 0, 60, 60),
            (240, 0, 64, 56),
        ]
    );
    assert_eq!(render.traces.len(), 4);
    assert!(render.traces.iter().all(|trace| trace.repeat <= 2));
}

#[test]
fn note_echo_applies_pitch_drift_scale_snap_and_channel_policy() {
    let input = vec![note_on_at(0, 60, 90, 0), note_off_at(120, 60, 0)];
    let mut config = NoteEchoConfig::new(TickTime::new(60, 480).expect("offset"));
    config.repeats = 2;
    config.feedback_count = 2;
    config.pitch_offset = 3;
    config.scale_snap = NoteEchoScaleSnap::major(0);
    config.channel_policy = NoteEchoChannelPolicy::Offset(1);
    let player = NoteEchoPlayer::new(config);

    let render = player.render(&input);

    assert_eq!(
        rendered_note_ons(&render.events),
        vec![(0, 0, 60, 90), (60, 1, 62, 90), (120, 2, 65, 90)]
    );
    assert_eq!(player.freeze(&input), render);
    assert_eq!(player.freeze(&input), player.render(&input));
}

#[test]
fn unknown_meta_and_sysex_preserve_bytes() {
    let bucket = MetaBucket {
        type_byte: 0x7f,
        data: vec![1, 2, 3, 4],
    };
    assert_eq!(MetaEvent::Other(bucket.clone()), MetaEvent::Other(bucket));
    let sysex_f0 = SysExEvent::F0 {
        data: vec![0x7d, 0x10, 0x11],
    };
    let sysex_f7 = SysExEvent::F7 {
        data: vec![0x01, 0x02],
    };
    match sysex_f0 {
        SysExEvent::F0 { data } => assert_eq!(data, vec![0x7d, 0x10, 0x11]),
        _ => unreachable!(),
    }
    match sysex_f7 {
        SysExEvent::F7 { data } => assert_eq!(data, vec![0x01, 0x02]),
        _ => unreachable!(),
    }
}

#[test]
fn meta_bucket_views_are_lossless() {
    let track_name = meta_view::make_track_name("piano");
    assert_eq!(meta_view::as_track_name(&track_name), Some("piano"));
    let marker = meta_view::make_marker("A");
    assert_eq!(meta_view::as_marker(&marker), Some("A"));
    let smpte = SmpteOffset {
        hours: 1,
        minutes: 2,
        seconds: 3,
        frames: 4,
        subframes: 5,
    };
    let bucket = meta_view::make_smpte_offset(smpte);
    assert_eq!(meta_view::as_smpte_offset(&bucket), Some(smpte));
}

#[test]
fn pump_quantizes_only_when_tpq_differs() {
    let event = MidiEvent {
        time: TickTime::new(1, 3).expect("tick time"),
        origin: synthetic_origin(),
        payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
    };
    let mut source = MemoryMidiSource::new(3, vec![event.clone()]);
    let mut same_tpq_sink = MemoryMidiSink::new(3);
    assert_eq!(pump(&mut source, &mut same_tpq_sink), Ok(1));
    assert_eq!(same_tpq_sink.events()[0].time, event.time);

    let mut source = MemoryMidiSource::new(3, vec![event]);
    let mut different_tpq_sink = MemoryMidiSink::new(480);
    assert_eq!(pump(&mut source, &mut different_tpq_sink), Ok(1));
    assert_eq!(
        different_tpq_sink.events()[0].time,
        TickTime::new(160, 480).expect("tick time")
    );
}

#[test]
fn tracked_sources_report_track_metadata() {
    let mut source = MemoryTrackedMidiSource::new(
        480,
        vec![
            TrackedMidiEvent {
                last_track: 2,
                event: MidiEvent {
                    time: TickTime::new(0, 480).expect("tick"),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
                },
            },
            TrackedMidiEvent {
                last_track: 5,
                event: MidiEvent {
                    time: TickTime::new(1, 480).expect("tick"),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Meta(MetaEvent::EndOfTrack),
                },
            },
        ],
    );
    assert_eq!(source.n_tracks(), 6);
    assert_eq!(source.last_track(), 0);
    let event = source
        .next_tracked()
        .unwrap_or_else(|never: Infallible| match never {});
    assert_eq!(event.expect("event").last_track, 2);
    assert_eq!(source.last_track(), 2);
}

#[test]
fn install_midi_io_lib_registers_runtime_exports() {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_midi_io_lib(&mut cx).expect("install");
    install_midi_io_lib(&mut cx).expect("install");
    let value = cx.resolve_value(&Symbol::qualified("midi", "MemoryMidiSource"));
    assert!(value.is_ok());
    let registry = cx
        .resolve_value(&Symbol::qualified("midi", "MidiIoRegistry"))
        .expect("registry");
    let expr = registry.object().as_expr(&mut cx).expect("expr");
    let Expr::Map(entries) = expr else {
        panic!("expected registry table");
    };
    assert!(entries.iter().any(|(key, value)| {
        *key == Expr::Symbol(Symbol::new("sources"))
            && matches!(value, Expr::List(items) if !items.is_empty())
    }));
}
