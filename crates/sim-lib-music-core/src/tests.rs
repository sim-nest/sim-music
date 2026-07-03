use num_rational::Ratio;

use crate::{
    Articulation, AtomRef, Channel, ChannelMessage, Counterpoint, Melody, MelodyItem, MidiEvent,
    MidiFileObj, MidiPayload, MidiTrackObj, Music, MusicError, MusicObject, Note, NoteEvent, Par,
    PianoRoll, PlayContext, PlayEvent, Playable, PlayableEvent, PlayableShape, Progression, Rest,
    Score, Seq, SmfFile, TickTime, Time, TimeRange, TimedNote, TraceEvent, stable_event_order,
    stable_lane_order, stream_envelopes,
};
use sim_kernel::Symbol;
use sim_lib_midi_core::{U7, synthetic_origin};
use sim_lib_midi_smf::{SmfFormat, SmfTrack};
use sim_lib_stream_core::{
    ClockDomain, STREAM_ENVELOPE_VERSION, StreamDirection, StreamMedia, StreamPacket,
};

fn quarter() -> Time {
    Ratio::new(1, 4)
}

fn note(midi: u8) -> Note {
    Note::new(
        quarter(),
        crate::Pitch::from_midi(midi),
        100,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn tick(ticks: i64) -> TickTime {
    TickTime::new(ticks, 480).expect("tick")
}

fn play_context(start: i64, end: i64) -> PlayContext {
    PlayContext::new(TimeRange::from_ticks(start, end, 480).expect("play range"))
}

#[test]
fn seq_and_par_durations() {
    let seq = Seq {
        children: vec![Box::new(note(60)), Box::new(note(64))],
    };
    let par = Par {
        children: vec![Box::new(note(60)), Box::new(note(64))],
    };
    assert_eq!(seq.duration(), Ratio::new(1, 2));
    assert_eq!(par.duration(), quarter());
}

#[test]
fn counterpoint_derives_voice_names() {
    let melody = Melody::new(vec![MelodyItem::Note(note(60))]).expect("melody");
    let counterpoint = Counterpoint::new(vec![melody], Vec::new()).expect("counterpoint");
    assert_eq!(counterpoint.voice_names, vec!["Voice 1".to_owned()]);
}

#[test]
fn voices_apply_offset() {
    let melody = Melody::new(vec![MelodyItem::Note(note(60))]).expect("melody");
    let mut out = Vec::new();
    melody.voices(Ratio::new(1, 2), &mut out);
    assert_eq!(out[0].onset, Ratio::new(1, 2));
}

#[test]
fn negative_duration_is_rejected() {
    let err = Rest::new(Ratio::new(-1, 4)).expect_err("negative duration");
    assert_eq!(err, MusicError::NegativeDuration);
}

#[test]
fn score_validates_tempo_and_time_signature() {
    assert_eq!(
        Score::new(
            0,
            (4, 4),
            None,
            Music::Rest(Rest::new(quarter()).expect("rest"))
        )
        .expect_err("tempo"),
        MusicError::InvalidTempo
    );
    assert_eq!(
        Score::new(
            120,
            (4, 0),
            None,
            Music::Rest(Rest::new(quarter()).expect("rest"))
        )
        .expect_err("time signature"),
        MusicError::InvalidTimeSignature
    );
}

#[test]
fn piano_roll_sorts_by_onset() {
    let later = TimedNote {
        onset: Ratio::new(1, 2),
        note: note(64),
    };
    let earlier = TimedNote {
        onset: Ratio::new(0, 1),
        note: note(60),
    };
    let roll = PianoRoll::new(vec![later, earlier]).expect("roll");
    assert_eq!(roll.items[0].note.pitch.to_midi(), Some(60));
}

#[test]
fn midi_track_object_emits_note_atoms() {
    let channel = Channel::new(0).expect("channel");
    let track = MidiTrackObj::new(
        vec![
            MidiEvent {
                time: TickTime::new(0, 480).expect("tick time"),
                origin: synthetic_origin(),
                payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                    ch: channel,
                    key: U7(60),
                    vel: U7(100),
                }),
            },
            MidiEvent {
                time: TickTime::new(480, 480).expect("tick time"),
                origin: synthetic_origin(),
                payload: MidiPayload::Channel(ChannelMessage::NoteOff {
                    ch: channel,
                    key: U7(60),
                    vel: U7(0),
                }),
            },
        ],
        Some(channel),
    );
    let mut out = Vec::new();
    track.voices(Time::from_integer(0), &mut out);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0].atom, AtomRef::Note(_)));
    assert_eq!(track.duration(), Ratio::new(1, 4));
}

#[test]
fn midi_file_object_uses_wrapped_tracks() {
    let channel = Channel::new(0).expect("channel");
    let file = SmfFile {
        format: SmfFormat::Simultaneous,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: vec![
                MidiEvent {
                    time: TickTime::new(0, 480).expect("tick time"),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                        ch: channel,
                        key: U7(60),
                        vel: U7(100),
                    }),
                },
                MidiEvent {
                    time: TickTime::new(240, 480).expect("tick time"),
                    origin: synthetic_origin(),
                    payload: MidiPayload::Channel(ChannelMessage::NoteOff {
                        ch: channel,
                        key: U7(60),
                        vel: U7(0),
                    }),
                },
            ],
        }],
    };
    let wrapped = MidiFileObj::new(file);
    let mut out = Vec::new();
    wrapped.voices(Time::from_integer(0), &mut out);
    assert_eq!(out.len(), 1);
}

#[test]
fn progression_and_score_are_music_objects() {
    let chord = crate::Chord::new(
        quarter(),
        "C",
        vec![crate::Pitch::from_midi(60), crate::Pitch::from_midi(64)],
        100,
        Channel::new(0).expect("channel"),
    )
    .expect("chord");
    let progression = Progression::new(None, vec![chord]).expect("progression");
    let score = Score::new(120, (4, 4), None, Music::Progression(progression)).expect("score");
    assert_eq!(score.duration(), quarter());
}

#[test]
fn playable_render_is_deterministic() {
    let music = Music::Melody(Melody::new(vec![MelodyItem::Note(note(60))]).expect("melody"));
    let cx = play_context(0, 960);

    let first = music.freeze(&cx).expect("first freeze");
    let second = music.freeze(&cx).expect("second freeze");

    assert_eq!(first.content_hash, second.content_hash);
    assert_eq!(first.events, second.events);
    assert_eq!(first.events.len(), 1);
    assert_eq!(first.events[0].time(), tick(0));
}

#[test]
fn playable_render_clips_range() {
    let music = Music::Note(note(60));
    let cx = play_context(120, 360);

    let frozen = music.freeze(&cx).expect("freeze");

    let [PlayEvent::Note(event)] = frozen.events.as_slice() else {
        panic!("expected one note event");
    };
    assert_eq!(event.time, tick(120));
    assert_eq!(event.duration, tick(240));
}

#[test]
fn playable_stream_envelopes_conform() {
    let music = Music::Note(note(60));
    let cx = play_context(0, 480);
    let stream = music.render_range(&cx).expect("stream");

    let envelopes = stream_envelopes(&stream).expect("envelopes");

    assert_eq!(envelopes.len(), 1);
    let envelope = &envelopes[0];
    assert_eq!(envelope.version(), STREAM_ENVELOPE_VERSION);
    assert_eq!(envelope.media(), StreamMedia::Data);
    assert_eq!(envelope.direction(), StreamDirection::Source);
    assert_eq!(envelope.clock_domain(), ClockDomain::MidiTick);
    assert_eq!(envelope.sequence(), 0);
    assert_eq!(envelope.ticks().len(), 1);
    assert_eq!(envelope.ticks()[0].clock, ClockDomain::MidiTick.symbol());
    assert!(envelope.clock_domains().contains(&ClockDomain::MidiTick));
    let StreamPacket::Data(packet) = envelope.packet() else {
        panic!("expected data packet");
    };
    assert_eq!(packet.kind, crate::play_event_data_kind());
}

#[test]
fn lane_target_validation_rejects_invalid_audio_target() {
    let err = crate::LaneDescriptor::new(
        crate::LaneId::new("audio"),
        crate::LaneKind::Audio,
        crate::LaneTarget::Instrument(Symbol::qualified("music/target", "synth")),
        0,
    )
    .expect_err("audio lanes need stream targets");

    assert!(matches!(err, MusicError::InvalidLaneTarget { .. }));
}

#[test]
fn stable_lane_and_event_order() {
    let lanes = stable_lane_order(vec![
        crate::LaneDescriptor::new(
            crate::LaneId::new("b"),
            crate::LaneKind::Note,
            crate::LaneTarget::Instrument(Symbol::qualified("music/target", "piano")),
            1,
        )
        .expect("lane b"),
        crate::LaneDescriptor::new(
            crate::LaneId::new("a"),
            crate::LaneKind::Control,
            crate::LaneTarget::Control(Symbol::qualified("music/control", "volume")),
            1,
        )
        .expect("lane a"),
        crate::LaneDescriptor::new(
            crate::LaneId::new("z"),
            crate::LaneKind::Trace,
            crate::LaneTarget::None,
            0,
        )
        .expect("lane z"),
    ]);
    assert_eq!(
        lanes
            .iter()
            .map(|lane| lane.id.as_ref())
            .collect::<Vec<_>>(),
        vec!["z", "a", "b"]
    );

    let channel = Channel::new(0).expect("channel");
    let mut events = vec![
        PlayEvent::Trace(TraceEvent {
            lane_id: crate::LaneId::new("z"),
            time: tick(20),
            step: 1,
        }),
        PlayEvent::Note(NoteEvent {
            lane_id: crate::LaneId::new("b"),
            time: tick(10),
            duration: tick(20),
            pitch: crate::Pitch::from_midi(60),
            velocity: 100,
            channel,
        }),
        PlayEvent::Playable(PlayableEvent {
            lane_id: crate::LaneId::new("a"),
            time: tick(10),
            playable: Symbol::qualified("music/playable", "child"),
        }),
    ];

    stable_event_order(&mut events);

    let ordered = events
        .iter()
        .map(|event| (event.time().ticks, event.lane_id().as_ref(), event.kind()))
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        vec![
            (10, "a", crate::LaneKind::Playable),
            (10, "b", crate::LaneKind::Note),
            (20, "z", crate::LaneKind::Trace),
        ]
    );
}

#[test]
fn playable_shape_round_trips() {
    let music = Music::Note(note(60));
    let shape = music.as_shape();
    let expr = shape.to_expr();

    assert_eq!(
        PlayableShape::from_expr(&expr).expect("shape round-trip"),
        shape
    );
}

#[test]
fn play_event_families_have_lane_kinds() {
    let channel = Channel::new(0).expect("channel");
    let midi_event = MidiEvent {
        time: tick(1),
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: channel,
            key: U7(60),
            vel: U7(100),
        }),
    };
    let lane = crate::LaneId::new("lane");
    let events = vec![
        PlayEvent::Note(NoteEvent {
            lane_id: lane.clone(),
            time: tick(1),
            duration: tick(2),
            pitch: crate::Pitch::from_midi(60),
            velocity: 100,
            channel,
        }),
        PlayEvent::Midi(crate::MidiPlayEvent {
            lane_id: lane.clone(),
            event: midi_event,
        }),
        PlayEvent::Pitch(crate::PitchEvent {
            lane_id: lane.clone(),
            time: tick(1),
            pitch: crate::Pitch::from_midi(61),
        }),
        PlayEvent::Control(crate::ControlEvent {
            lane_id: lane.clone(),
            time: tick(1),
            control: Symbol::qualified("music/control", "volume"),
            value: 64,
        }),
        PlayEvent::Audio(crate::AudioEvent {
            lane_id: lane.clone(),
            time: tick(1),
            frames: 128,
        }),
        PlayEvent::Playable(PlayableEvent {
            lane_id: lane.clone(),
            time: tick(1),
            playable: Symbol::qualified("music/playable", "child"),
        }),
        PlayEvent::Performance(crate::PerformanceEvent {
            lane_id: lane.clone(),
            source_id: Symbol::qualified("music/performance-source", "test"),
            input_time: tick(1),
            time: tick(1),
            intent: crate::PerformanceIntent::Parameter {
                target: Symbol::qualified("music/performance", "start"),
                value: 1,
            },
        }),
        PlayEvent::Diagnostic(crate::DiagnosticEvent {
            lane_id: lane.clone(),
            time: tick(1),
            message: "ok".to_owned(),
        }),
        PlayEvent::Trace(TraceEvent {
            lane_id: lane,
            time: tick(1),
            step: 7,
        }),
    ];

    assert_eq!(
        events.iter().map(PlayEvent::kind).collect::<Vec<_>>(),
        vec![
            crate::LaneKind::Note,
            crate::LaneKind::Midi,
            crate::LaneKind::Pitch,
            crate::LaneKind::Control,
            crate::LaneKind::Audio,
            crate::LaneKind::Playable,
            crate::LaneKind::Performance,
            crate::LaneKind::Diagnostic,
            crate::LaneKind::Trace,
        ]
    );
}
