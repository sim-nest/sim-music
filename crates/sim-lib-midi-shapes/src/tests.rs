use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol, read_construct_capability};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaBucket, MetaEvent, MidiEvent, MidiPayload, RawBytes, SysExEvent,
    TickTime, U7, U14, synthetic_origin,
};
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTrack};
use std::sync::Arc;

use crate::{
    MidiChannelMessageDescriptor, MidiEventDescriptor, MidiMetaEventDescriptor,
    MidiSmfFileDescriptor, MidiSmfTrackDescriptor, decode_channel_message, decode_meta_event,
    decode_midi_event, decode_smf_file, decode_smf_track, decode_sysex, decode_tick_time,
    decode_tick_time_with_tpq, encode_channel_message, encode_meta_event, encode_midi_event,
    encode_smf_file, encode_smf_track, encode_sysex, encode_tick_time, install_midi_shapes_lib,
    midi_channel_message_class_symbol, midi_event_class_symbol, midi_meta_event_class_symbol,
    midi_smf_file_class_symbol, midi_smf_track_class_symbol,
};

#[test]
fn tick_time_round_trip() {
    let time = TickTime::new(960, 480).expect("tick time");
    assert_eq!(
        decode_tick_time(&encode_tick_time(time)).expect("decode"),
        time
    );
}

#[test]
fn quarter_sugar_uses_inherited_tpq() {
    let time = decode_tick_time_with_tpq("2q", Some(480)).expect("decode");
    assert_eq!(time, TickTime::new(960, 480).expect("tick time"));
    let half = decode_tick_time_with_tpq("3/2q", Some(480)).expect("decode");
    assert_eq!(half, TickTime::new(720, 480).expect("tick time"));
}

#[test]
fn channel_messages_round_trip() {
    let messages = [
        ChannelMessage::NoteOff {
            ch: Channel::new(0).expect("channel"),
            key: U7(60),
            vel: U7(64),
        },
        ChannelMessage::NoteOn {
            ch: Channel::new(1).expect("channel"),
            key: U7(61),
            vel: U7(96),
        },
        ChannelMessage::PolyAftertouch {
            ch: Channel::new(2).expect("channel"),
            key: U7(62),
            pressure: U7(32),
        },
        ChannelMessage::ControlChange {
            ch: Channel::new(3).expect("channel"),
            cc: U7(7),
            value: U7(100),
        },
        ChannelMessage::ProgramChange {
            ch: Channel::new(4).expect("channel"),
            program: U7(12),
        },
        ChannelMessage::ChanAftertouch {
            ch: Channel::new(5).expect("channel"),
            pressure: U7(77),
        },
        ChannelMessage::PitchBend {
            ch: Channel::new(6).expect("channel"),
            value: U14(8192),
        },
    ];
    for message in messages {
        let encoded = encode_channel_message(message);
        let decoded = decode_channel_message(&encoded).expect("decode");
        assert_eq!(decoded, message);
    }
}

#[test]
fn unknown_meta_data_round_trips_through_meta_bucket() {
    let meta = MetaEvent::Other(MetaBucket {
        type_byte: 0x7f,
        data: vec![0xde, 0xad, 0xbe, 0xef],
    });
    let encoded = encode_meta_event(&meta);
    let decoded = decode_meta_event(&encoded).expect("decode");
    assert_eq!(decoded, meta);
}

#[test]
fn sysex_payloads_preserve_bytes() {
    let f0 = SysExEvent::F0 {
        data: vec![0x7d, 0x01, 0x02, 0x03],
    };
    let f7 = SysExEvent::F7 {
        data: vec![0x55, 0xaa],
    };
    assert_eq!(decode_sysex(&encode_sysex(&f0)).expect("decode"), f0);
    assert_eq!(decode_sysex(&encode_sysex(&f7)).expect("decode"), f7);
}

#[test]
fn midi_event_round_trip() {
    let event = MidiEvent {
        time: TickTime::new(240, 480).expect("tick time"),
        origin: synthetic_origin(),
        payload: MidiPayload::Raw(RawBytes {
            status: 0xf4,
            data: vec![1, 2, 3],
        }),
    };
    let encoded = encode_midi_event(&event);
    let decoded = decode_midi_event(&encoded).expect("decode");
    assert_eq!(decoded.time, event.time);
    assert_eq!(decoded.payload, event.payload);
}

#[test]
fn smf_track_round_trip() {
    let track = SmfTrack {
        events: vec![MidiEvent {
            time: TickTime::new(0, 480).expect("tick time"),
            origin: synthetic_origin(),
            payload: MidiPayload::Meta(MetaEvent::Tempo {
                us_per_quarter: 500_000,
            }),
        }],
    };
    let encoded = encode_smf_track(&track);
    let decoded = decode_smf_track(&encoded).expect("decode");
    assert_eq!(decoded, track);
}

#[test]
fn smf_file_round_trip() {
    let file = SmfFile {
        format: SmfFormat::Simultaneous,
        tpq: 480,
        tracks: vec![
            SmfTrack {
                events: vec![MidiEvent {
                    time: TickTime::new(0, 480).expect("tick time"),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Meta(MetaEvent::Tempo {
                        us_per_quarter: 500_000,
                    }),
                }],
            },
            SmfTrack {
                events: vec![MidiEvent {
                    time: TickTime::new(120, 480).expect("tick time"),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                        ch: Channel::new(0).expect("channel"),
                        key: U7(60),
                        vel: U7(100),
                    }),
                }],
            },
        ],
    };
    let encoded = encode_smf_file(&file);
    let decoded = decode_smf_file(&encoded).expect("decode");
    assert_eq!(decoded, file);
}

#[test]
fn install_midi_shapes_lib_registers_runtime_shape_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_midi_shapes_lib(&mut cx).unwrap();
    install_midi_shapes_lib(&mut cx).unwrap();
    let shape = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("midi", "MidiEvent"))
        .expect("midi event shape")
        .clone();
    let doc = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .describe(&mut cx)
        .unwrap();
    assert_eq!(doc.name, "MidiEvent");
}

#[test]
fn midi_shapes_reject_invalid_values() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_midi_shapes_lib(&mut cx).unwrap();

    let tick = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("midi", "TickTime"))
        .expect("tick-time shape")
        .clone();
    let tick_shape = tick.object().as_shape().expect("shape protocol");
    assert!(!tick_shape.is_total());
    assert!(
        tick_shape
            .check_expr(&mut cx, &Expr::String("2q".to_owned()))
            .unwrap()
            .accepted
    );
    assert!(
        !tick_shape
            .check_expr(&mut cx, &Expr::Bool(false))
            .unwrap()
            .accepted
    );

    let channel = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("midi", "ChannelMessage"))
        .expect("channel-message shape")
        .clone();
    let channel_shape = channel.object().as_shape().expect("shape protocol");
    assert!(
        channel_shape
            .check_expr(
                &mut cx,
                &Expr::String("#(Channel NoteOn 0 60 100)".to_owned())
            )
            .unwrap()
            .accepted
    );
    assert!(
        !channel_shape
            .check_expr(&mut cx, &Expr::String("#(Meta Tempo 500000)".to_owned()))
            .unwrap()
            .accepted
    );
}

#[test]
fn midi_citizens_accept_legacy_text_and_read_construct() {
    let mut cx = cx_with_citizens();

    let channel_text = "#(Channel NoteOn 0 60 100)";
    let channel = read_construct::<MidiChannelMessageDescriptor>(
        &mut cx,
        midi_channel_message_class_symbol(),
        channel_text,
    );
    assert_eq!(
        channel.message().unwrap(),
        decode_channel_message(channel_text).unwrap()
    );

    let meta_text = "#(Meta Tempo 500000)";
    let meta = read_construct::<MidiMetaEventDescriptor>(
        &mut cx,
        midi_meta_event_class_symbol(),
        meta_text,
    );
    assert_eq!(meta.event().unwrap(), decode_meta_event(meta_text).unwrap());

    let event_text = "#(MidiEvent #(TickTime 0 480) #(Channel NoteOn 0 60 100))";
    let event =
        read_construct::<MidiEventDescriptor>(&mut cx, midi_event_class_symbol(), event_text);
    assert_eq!(
        event.event().unwrap().payload,
        decode_midi_event(event_text).unwrap().payload
    );
    assert_eq!(
        MidiEventDescriptor::read_construct_expr_from_text(event_text).unwrap(),
        read_construct_expr(midi_event_class_symbol(), event.as_text())
    );

    let track_text = "#(SmfTrack #(MidiEvent #(TickTime 0 480) #(Meta Tempo 500000)))";
    let track = read_construct::<MidiSmfTrackDescriptor>(
        &mut cx,
        midi_smf_track_class_symbol(),
        track_text,
    );
    assert_eq!(
        track.track().unwrap(),
        decode_smf_track(track_text).unwrap()
    );

    let file_text = "#(SmfFile SingleTrack 480 #(SmfTrack))";
    let file =
        read_construct::<MidiSmfFileDescriptor>(&mut cx, midi_smf_file_class_symbol(), file_text);
    assert_eq!(file.file().unwrap(), decode_smf_file(file_text).unwrap());
}

fn cx_with_citizens() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.load_lib(&sim_citizen::CitizenLib::all()).unwrap();
    cx.grant(read_construct_capability());
    cx
}

fn read_construct<T>(cx: &mut Cx, class: Symbol, form: &str) -> T
where
    T: Clone + 'static,
{
    let args = [
        Expr::Symbol(Symbol::new("v1")),
        Expr::String(form.to_owned()),
    ]
    .iter()
    .map(|expr| sim_citizen::value_from_expr(cx, expr))
    .collect::<sim_kernel::Result<Vec<_>>>()
    .unwrap();
    cx.read_construct(&class, args)
        .unwrap()
        .object()
        .downcast_ref::<T>()
        .unwrap()
        .clone()
}

fn read_construct_expr(class: Symbol, form: &str) -> Expr {
    Expr::Extension {
        tag: Symbol::qualified("citizen", "read-construct"),
        payload: Box::new(Expr::Vector(vec![
            Expr::Symbol(class),
            Expr::Symbol(Symbol::new("v1")),
            Expr::String(form.to_owned()),
        ])),
    }
}
