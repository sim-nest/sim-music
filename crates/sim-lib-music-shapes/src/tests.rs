use num_rational::Ratio;
use std::any::Any;
use std::sync::Arc;

mod custom_filter;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol, Value, read_construct_capability};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MidiEvent, MidiPayload, TickTime, U7, synthetic_origin,
};
use sim_lib_midi_smf::{SmfFile, SmfFormat, SmfTrack};
use sim_lib_music_analysis::{ChordWindowMode, DiffRoll, chord_windows_from_piano_roll};
use sim_lib_music_core::{
    Arranger, ArrangerPlacement, Articulation, Chord, Counterpoint, LaneId, Melody, MelodyItem,
    MidiFileObj, MidiTrackObj, Music, MusicObject, Note, Par, PianoRoll, PitchRemap, PlayableRef,
    Progression, Rest, Score, Seq, StretchPolicy, TimedNote,
};
use sim_lib_music_lift::{
    CounterpointLiftOpts, LabelStrategy, ProgressionLiftOpts, VoiceAssignment,
};
use sim_lib_music_transform::{FunctionMap, RetrogradeMode};
use sim_lib_pitch_scale::{Mode, Scale};

use crate::{
    MusicChordDescriptor, MusicMelodyDescriptor, MusicNoteDescriptor, MusicParDescriptor,
    MusicScoreDescriptor, MusicSeqDescriptor, MusicShapeError, decode_arranger, decode_chord,
    decode_chord_window, decode_chord_window_mode, decode_counterpoint,
    decode_counterpoint_lift_opts, decode_diff_roll, decode_function_map, decode_label_strategy,
    decode_melody, decode_midi_file, decode_midi_track, decode_music, decode_music_file,
    decode_note, decode_piano_roll, decode_progression, decode_progression_lift_opts, decode_rest,
    decode_retrograde_mode, decode_score, decode_voice_assignment, encode_arranger, encode_chord,
    encode_chord_window, encode_chord_window_mode, encode_counterpoint,
    encode_counterpoint_lift_opts, encode_diff_roll, encode_function_map, encode_label_strategy,
    encode_melody, encode_midi_file, encode_midi_track, encode_music, encode_music_file,
    encode_note, encode_par, encode_piano_roll, encode_progression, encode_progression_lift_opts,
    encode_rest, encode_retrograde_mode, encode_score, encode_seq, encode_voice_assignment,
    install_music_shapes_lib, music_chord_class_symbol, music_melody_class_symbol,
    music_note_class_symbol, music_par_class_symbol, music_score_class_symbol,
    music_seq_class_symbol,
};

fn quarter() -> Ratio<i64> {
    Ratio::new(1, 4)
}

fn note(midi: u8) -> Note {
    Note::new(
        quarter(),
        sim_lib_music_core::Pitch::from_midi(midi),
        100,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn encoded_arranger(arranger: &Arranger) -> String {
    encode_arranger(arranger).expect("encode arranger")
}

fn encoded_music(music: &Music) -> String {
    encode_music(music).expect("encode music")
}

fn encoded_music_file(score: &Score) -> String {
    encode_music_file(score).expect("encode music file")
}

fn encoded_par(par: &Par) -> String {
    encode_par(par).expect("encode par")
}

fn encoded_score(score: &Score) -> String {
    encode_score(score).expect("encode score")
}

fn encoded_seq(seq: &Seq) -> String {
    encode_seq(seq).expect("encode seq")
}

#[test]
fn leaf_music_objects_round_trip() {
    let note_value = note(60);
    assert_eq!(
        decode_note(&encode_note(&note_value)).expect("note"),
        note_value
    );

    let rest_value = Rest::new(quarter()).expect("rest");
    assert_eq!(
        decode_rest(&encode_rest(&rest_value)).expect("rest"),
        rest_value
    );

    let chord_value = Chord::new(
        quarter(),
        "C:maj",
        vec![
            sim_lib_music_core::Pitch::from_midi(60),
            sim_lib_music_core::Pitch::from_midi(64),
        ],
        100,
        Channel::new(0).expect("channel"),
    )
    .expect("chord");
    assert_eq!(
        decode_chord(&encode_chord(&chord_value)).expect("chord"),
        chord_value
    );
}

#[test]
fn structured_music_objects_round_trip() {
    let melody_value = Melody::new(vec![
        MelodyItem::Note(note(60)),
        MelodyItem::Rest(Rest::new(quarter()).expect("rest")),
    ])
    .expect("melody");
    assert_eq!(
        decode_melody(&encode_melody(&melody_value)).expect("melody"),
        melody_value
    );

    let progression_value = Progression::new(
        Some("C-major".to_owned()),
        vec![
            Chord::new(
                quarter(),
                "I",
                vec![sim_lib_music_core::Pitch::from_midi(60)],
                100,
                Channel::new(0).expect("channel"),
            )
            .expect("chord"),
        ],
    )
    .expect("progression");
    assert_eq!(
        decode_progression(&encode_progression(&progression_value)).expect("progression"),
        progression_value
    );

    let counterpoint_value = Counterpoint::new(vec![melody_value.clone()], Vec::new()).expect("cp");
    assert_eq!(
        decode_counterpoint(&encode_counterpoint(&counterpoint_value)).expect("counterpoint"),
        counterpoint_value
    );

    let roll_value = PianoRoll::new(vec![TimedNote {
        onset: Ratio::new(0, 1),
        note: note(60),
    }])
    .expect("roll");
    assert_eq!(
        decode_piano_roll(&encode_piano_roll(&roll_value)).expect("roll"),
        roll_value
    );
}

#[derive(Clone)]
struct UnsupportedMusicObject;

impl MusicObject for UnsupportedMusicObject {
    fn kind(&self) -> &'static str {
        "unsupported-test"
    }

    fn duration(&self) -> Ratio<i64> {
        Ratio::from_integer(0)
    }

    fn voices<'a>(
        &'a self,
        _offset: Ratio<i64>,
        _out: &mut Vec<sim_lib_music_core::TimedAtom<'a>>,
    ) {
    }

    fn clone_box(&self) -> Box<dyn MusicObject> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn composite_encoder_rejects_unsupported_public_music_object() {
    let par = Par {
        children: vec![Box::new(UnsupportedMusicObject)],
    };

    assert!(matches!(
        encode_par(&par),
        Err(MusicShapeError::UnsupportedMusicObject("unsupported-test"))
    ));
}

#[test]
fn midi_wrappers_round_trip() {
    let track = MidiTrackObj::new(
        vec![MidiEvent {
            time: TickTime::new(0, 480).expect("tick"),
            origin: synthetic_origin(),
            payload: MidiPayload::Channel(ChannelMessage::NoteOn {
                ch: Channel::new(0).expect("channel"),
                key: U7(60),
                vel: U7(100),
            }),
        }],
        Some(Channel::new(0).expect("channel")),
    );
    assert_eq!(
        decode_midi_track(&encode_midi_track(&track)).expect("track"),
        track
    );

    let file = MidiFileObj::new(SmfFile {
        format: SmfFormat::SingleTrack,
        tpq: 480,
        tracks: vec![SmfTrack {
            events: track.events.clone(),
        }],
    });
    assert_eq!(
        decode_midi_file(&encode_midi_file(&file)).expect("file"),
        file
    );
}

#[test]
fn par_seq_music_and_score_round_trip_via_canonical_text() {
    let par_value = Music::Par(Par {
        children: vec![Box::new(note(60)), Box::new(note(64))],
    });
    let seq_value = Music::Seq(Seq {
        children: vec![
            Box::new(note(60)),
            Box::new(Rest::new(quarter()).expect("rest")),
        ],
    });
    assert_eq!(
        encoded_music(&decode_music(&encoded_music(&par_value)).expect("par")),
        encoded_music(&par_value)
    );
    assert_eq!(
        encoded_music(&decode_music(&encoded_music(&seq_value)).expect("seq")),
        encoded_music(&seq_value)
    );

    let score = Score::new(120, (4, 4), Some("C-major".to_owned()), par_value).expect("score");
    assert_eq!(
        encoded_score(&decode_score(&encoded_score(&score)).expect("score")),
        encoded_score(&score)
    );
    assert_eq!(
        encoded_music_file(&decode_music_file(&encoded_music_file(&score)).expect("music file")),
        encoded_music_file(&score)
    );
}

#[test]
fn arranger_round_trips_via_canonical_text() {
    let arranger = Arranger::new(
        vec![
            ArrangerPlacement::new(
                "note",
                PlayableRef::inline(Music::Note(note(60))),
                Ratio::new(1, 4),
            )
            .expect("placement")
            .with_duration(Ratio::new(1, 4))
            .expect("duration")
            .with_stretch(StretchPolicy::TimeRatio(Ratio::new(2, 1)))
            .with_pitch_remap(PitchRemap::Chromatic(1)),
        ],
        vec![LaneId::new("notes")],
    )
    .expect("arranger");

    assert_eq!(
        encoded_arranger(&decode_arranger(&encoded_arranger(&arranger)).expect("arranger")),
        encoded_arranger(&arranger)
    );
    assert_eq!(
        encoded_music(&decode_music(&encoded_music(&Music::Arranger(arranger.clone()))).unwrap()),
        encoded_music(&Music::Arranger(arranger))
    );
}

#[test]
fn install_music_shapes_lib_registers_runtime_shape_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_shapes_lib(&mut cx).unwrap();
    install_music_shapes_lib(&mut cx).unwrap();
    let shape = cx
        .registry()
        .shape_by_symbol(&Symbol::qualified("music", "Score"))
        .expect("score shape")
        .clone();
    let doc = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .describe(&mut cx)
        .unwrap();
    assert_eq!(doc.name, "Score");
}

#[test]
fn music_runtime_shapes_reject_bad_domain_forms() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_shapes_lib(&mut cx).unwrap();

    let note_value = note(60);
    let note_shape = registered_music_shape(&cx, "Note");
    assert_shape_accepts(&mut cx, &note_shape, &encode_note(&note_value));
    assert!(!note_shape.object().as_shape().unwrap().is_total());
    assert_shape_rejects(
        &mut cx,
        &note_shape,
        "#(Note pitch=C4 vel=100 channel=0 articulation=Normal)",
    );
    assert_shape_rejects(&mut cx, &note_shape, "#(Rest dur=1/4)");
    assert_shape_rejects(
        &mut cx,
        &note_shape,
        "#(Note dur=#(Rest dur=1/4) pitch=C4 vel=100 channel=0 articulation=Normal)",
    );

    let rest_shape = registered_music_shape(&cx, "Rest");
    assert_shape_accepts(
        &mut cx,
        &rest_shape,
        &encode_rest(&Rest::new(quarter()).expect("rest")),
    );
    assert_shape_rejects(&mut cx, &rest_shape, "#(Rest)");
    assert_shape_rejects(&mut cx, &rest_shape, &encode_note(&note_value));

    let score = Score::new(120, (4, 4), None, Music::Note(note_value)).expect("score");
    let score_shape = registered_music_shape(&cx, "Score");
    assert_shape_accepts(&mut cx, &score_shape, &encoded_score(&score));
    assert_shape_rejects(
        &mut cx,
        &score_shape,
        "#(Score tempo=120 time_sig=4/4 key=none)",
    );
    assert_shape_rejects(
        &mut cx,
        &score_shape,
        "#(Score tempo=120 time_sig=4/4 key=none body=[#(Rest dur=1/4)])",
    );
}

#[test]
fn music_citizens_accept_legacy_text_and_read_construct() {
    let mut cx = cx_with_citizens();

    let note_text = "#(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal)";
    let note = read_construct::<MusicNoteDescriptor>(&mut cx, music_note_class_symbol(), note_text);
    assert_eq!(note.note().unwrap(), decode_note(note_text).unwrap());
    assert_eq!(
        MusicNoteDescriptor::read_construct_expr_from_text(note_text).unwrap(),
        read_construct_expr(music_note_class_symbol(), note.as_text())
    );

    let seq_text =
        "#(Seq children=[#(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal)])";
    let seq = read_construct::<MusicSeqDescriptor>(&mut cx, music_seq_class_symbol(), seq_text);
    assert_eq!(seq.as_text(), encoded_seq(&seq.seq().unwrap()));

    let par_text =
        "#(Par children=[#(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal)])";
    let par = read_construct::<MusicParDescriptor>(&mut cx, music_par_class_symbol(), par_text);
    assert_eq!(par.as_text(), encoded_par(&par.par().unwrap()));

    let chord_text = "#(Chord dur=1/4 symbol=\"C\" pitches=[C4,E4,G4] vel=100 channel=0)";
    let chord =
        read_construct::<MusicChordDescriptor>(&mut cx, music_chord_class_symbol(), chord_text);
    assert_eq!(chord.chord().unwrap(), decode_chord(chord_text).unwrap());

    let melody_text =
        "#(Melody items=[#(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal)])";
    let melody =
        read_construct::<MusicMelodyDescriptor>(&mut cx, music_melody_class_symbol(), melody_text);
    assert_eq!(
        melody.melody().unwrap(),
        decode_melody(melody_text).unwrap()
    );

    let score_text = "#(Score tempo=120 time_sig=4/4 key=none body=#(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal))";
    let score =
        read_construct::<MusicScoreDescriptor>(&mut cx, music_score_class_symbol(), score_text);
    assert_eq!(
        encoded_score(&score.score().unwrap()),
        encoded_score(&decode_score(score_text).unwrap())
    );
}

fn cx_with_citizens() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.load_lib(&sim_citizen::CitizenLib::all()).unwrap();
    cx.grant(read_construct_capability());
    cx
}

fn registered_music_shape(cx: &Cx, name: &'static str) -> Value {
    cx.registry()
        .shape_by_symbol(&Symbol::qualified("music", name))
        .expect("registered music shape")
        .clone()
}

fn assert_shape_accepts(cx: &mut Cx, shape: &Value, text: &str) {
    let expr = Expr::String(text.to_owned());
    let matched = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .check_expr(cx, &expr)
        .unwrap();
    assert!(
        matched.accepted,
        "{text} rejected: {:?}",
        matched.diagnostics
    );
}

fn assert_shape_rejects(cx: &mut Cx, shape: &Value, text: &str) {
    let expr = Expr::String(text.to_owned());
    let matched = shape
        .object()
        .as_shape()
        .expect("shape protocol")
        .check_expr(cx, &expr)
        .unwrap();
    assert!(
        !matched.accepted,
        "{text} unexpectedly matched with score {:?}",
        matched.score
    );
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

#[test]
fn diff_roll_and_transform_option_values_round_trip() {
    let roll = PianoRoll::new(vec![
        TimedNote {
            onset: Ratio::new(0, 1),
            note: note(60),
        },
        TimedNote {
            onset: Ratio::new(1, 4),
            note: note(64),
        },
    ])
    .expect("roll");
    let diff = DiffRoll::from_piano_roll(&roll);
    assert_eq!(
        decode_diff_roll(&encode_diff_roll(&diff)).expect("diff"),
        diff
    );
    assert_eq!(
        decode_retrograde_mode(&encode_retrograde_mode(RetrogradeMode::PinnedNoteOn))
            .expect("retrograde"),
        RetrogradeMode::PinnedNoteOn
    );
    let custom = FunctionMap::Custom(Scale::new(
        sim_lib_music_core::Pitch::from_midi(60).class,
        Mode::Dorian,
    ));
    assert_eq!(
        decode_function_map(&encode_function_map(&custom)).expect("function map"),
        custom
    );
    assert_eq!(
        decode_chord_window_mode(&encode_chord_window_mode(ChordWindowMode::StartingNotes))
            .expect("window mode"),
        ChordWindowMode::StartingNotes
    );
}

#[test]
fn chord_window_round_trips() {
    let roll = PianoRoll::new(vec![
        TimedNote {
            onset: Ratio::new(0, 1),
            note: note(60),
        },
        TimedNote {
            onset: Ratio::new(0, 1),
            note: note(64),
        },
        TimedNote {
            onset: Ratio::new(1, 4),
            note: note(67),
        },
    ])
    .expect("roll");
    let windows = chord_windows_from_piano_roll(&roll, ChordWindowMode::SoundingNotes);
    let first = windows.first().expect("window");
    assert_eq!(
        decode_chord_window(&encode_chord_window(first)).expect("window"),
        *first
    );
}

#[test]
fn lift_option_values_round_trip() {
    assert_eq!(
        decode_label_strategy(&encode_label_strategy(LabelStrategy::SetClass)).expect("strategy"),
        LabelStrategy::SetClass
    );
    assert_eq!(
        decode_voice_assignment(&encode_voice_assignment(VoiceAssignment::TrackThenChannel))
            .expect("assignment"),
        VoiceAssignment::TrackThenChannel
    );

    let progression_opts = ProgressionLiftOpts {
        grid: Ratio::new(1, 8),
        min_notes: 3,
        key_hint: Some(sim_lib_pitch_scale::Key {
            tonic: sim_lib_music_core::Pitch::from_midi(60).class,
            mode: Mode::Dorian,
        }),
        label_strategy: LabelStrategy::Functional,
        window_mode: ChordWindowMode::StartingNotes,
    };
    assert_eq!(
        decode_progression_lift_opts(&encode_progression_lift_opts(&progression_opts))
            .expect("progression opts"),
        progression_opts
    );

    let counterpoint_opts = CounterpointLiftOpts {
        min_rest_to_close: Ratio::new(1, 32),
        max_voices_per_track: 6,
        voice_assignment: VoiceAssignment::HighestFirst,
    };
    assert_eq!(
        decode_counterpoint_lift_opts(&encode_counterpoint_lift_opts(&counterpoint_opts))
            .expect("counterpoint opts"),
        counterpoint_opts
    );
}
