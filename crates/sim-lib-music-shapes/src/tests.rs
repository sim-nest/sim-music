use num_rational::Ratio;
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

mod custom_filter;
mod serial_plan;

use sim_codec::{Input, decode_eval_expr_with_codec, decode_with_codec, encode_value_with_codec};
use sim_codec_json::JsonCodecLib;
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{
    CapabilitySet, Cx, DefaultFactory, EagerPolicy, EncodeOptions, Expr, ReadPolicy, Symbol,
    TrustLevel, Value, read_construct_capability,
};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MidiEvent, MidiPayload, TickTime, U7, synthetic_origin,
};
use sim_lib_midi_smf::{SmfDivision, SmfFile, SmfFormat, SmfTrack};
use sim_lib_music_analysis::{ChordWindowMode, DiffRoll, chord_windows_from_piano_roll};
use sim_lib_music_core::{
    Arranger, ArrangerPlacement, Articulation, Chord, Counterpoint, LaneId, Melody, MelodyItem,
    MidiFileObj, MidiTrackObj, Music, MusicObject, Note, Par, PianoRoll, PitchRemap, PlayableRef,
    Progression, Rest, Score, Seq, StretchPolicy, TimedNote,
};
use sim_lib_music_lift::{
    CounterpointLiftOpts, DanglingNotePolicy, LabelStrategy, MidiRealizationPolicy, OverlapPolicy,
    PedalPolicy, ProgressionLiftOpts, SameTickPolicy, VoiceAssignment,
};
use sim_lib_music_serial::{
    EventPlacement, OrdinalRef, PlannedSerialEvent, RowInstanceId, SerialEventId, SerialOrigin,
    SerialPlan, SerialRole, StructuralLicense, StructuralReadingId,
};
use sim_lib_music_transform::{FunctionMap, RetrogradeMode};
use sim_lib_pitch_scale::{Mode, Scale};
use sim_lib_pitch_serial::{RowFamily, RowOperation, ToneRow};

use crate::{
    MusicChordDescriptor, MusicMelodyDescriptor, MusicNoteDescriptor, MusicParDescriptor,
    MusicScoreDescriptor, MusicSeqDescriptor, MusicShapeError, decode_arranger, decode_chord,
    decode_chord_window, decode_chord_window_mode, decode_counterpoint,
    decode_counterpoint_lift_opts, decode_diff_roll, decode_function_map, decode_label_strategy,
    decode_melody, decode_midi_file, decode_midi_realization_policy, decode_midi_track,
    decode_music, decode_music_file, decode_note, decode_piano_roll, decode_progression,
    decode_progression_lift_opts, decode_rest, decode_retrograde_mode, decode_score,
    decode_serial_series, decode_voice_assignment, encode_arranger, encode_chord,
    encode_chord_window, encode_chord_window_mode, encode_counterpoint,
    encode_counterpoint_lift_opts, encode_diff_roll, encode_function_map, encode_label_strategy,
    encode_melody, encode_midi_file, encode_midi_realization_policy, encode_midi_track,
    encode_music, encode_music_file, encode_note, encode_par, encode_piano_roll,
    encode_progression, encode_progression_lift_opts, encode_rest, encode_retrograde_mode,
    encode_score, encode_seq, encode_serial_plan, encode_serial_series, encode_voice_assignment,
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

fn serial_realization_request_expr() -> Expr {
    let row = ToneRow::try_from_classes([
        sim_lib_music_core::PitchClass::E,
        sim_lib_music_core::PitchClass::F,
        sim_lib_music_core::PitchClass::G,
        sim_lib_music_core::PitchClass::CS,
        sim_lib_music_core::PitchClass::FS,
        sim_lib_music_core::PitchClass::DS,
        sim_lib_music_core::PitchClass::GS,
        sim_lib_music_core::PitchClass::D,
        sim_lib_music_core::PitchClass::B,
        sim_lib_music_core::PitchClass::C,
        sim_lib_music_core::PitchClass::A,
        sim_lib_music_core::PitchClass::AS,
    ])
    .expect("row")
    .apply(RowOperation::new(RowFamily::P, 0));
    let row_id = RowInstanceId::new("row/lisp/p0").expect("row id");
    let mut rows = BTreeMap::new();
    rows.insert(row_id.clone(), row);
    let license = StructuralLicense::new(
        StructuralReadingId::new("reading/lisp").expect("reading id"),
        "lisp runtime request",
    )
    .expect("license");
    let event_id = SerialEventId::new("event/statement").expect("event id");
    let event = PlannedSerialEvent {
        id: event_id.clone(),
        ordinals: (0..12)
            .map(|ordinal| OrdinalRef::new(row_id.clone(), ordinal))
            .collect(),
        role: SerialRole::Structural,
        origin: SerialOrigin::Structural {
            rationale: "single statement".to_owned(),
        },
        voice: sim_lib_music_core::ObjectId::new("voice/high").expect("voice"),
        placement: EventPlacement::independent(),
        parents: vec![],
        licenses: vec![license],
    };
    let plan = SerialPlan::try_new(
        rows,
        [(event_id.clone(), event)].into_iter().collect(),
        std::iter::empty::<(SerialEventId, SerialEventId)>(),
    )
    .expect("plan");
    let plan = encode_serial_plan(&plan).expect("encode plan");

    Expr::Map(vec![
        (Expr::Symbol(Symbol::new("plan")), Expr::String(plan)),
        (
            Expr::Symbol(Symbol::new("context")),
            Expr::Map(vec![
                (
                    Expr::Symbol(Symbol::new("specs")),
                    Expr::Vector(vec![Expr::Map(vec![
                        (
                            Expr::Symbol(Symbol::new("id")),
                            Expr::String("event/statement".to_owned()),
                        ),
                        (
                            Expr::Symbol(Symbol::new("sound")),
                            Expr::String("notes".to_owned()),
                        ),
                        (
                            Expr::Symbol(Symbol::new("register")),
                            Expr::String("4".to_owned()),
                        ),
                        (
                            Expr::Symbol(Symbol::new("duration")),
                            Expr::String("1/4".to_owned()),
                        ),
                        (
                            Expr::Symbol(Symbol::new("velocity")),
                            Expr::String("96".to_owned()),
                        ),
                        (
                            Expr::Symbol(Symbol::new("channel")),
                            Expr::String("0".to_owned()),
                        ),
                        (
                            Expr::Symbol(Symbol::new("articulation")),
                            Expr::String("Normal".to_owned()),
                        ),
                    ])]),
                ),
                (
                    Expr::Symbol(Symbol::new("scale")),
                    Expr::String("C:dorian".to_owned()),
                ),
            ]),
        ),
    ])
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
        division: SmfDivision::metrical(480).unwrap(),
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
    let mut cx = Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        sim_kernel::HandleSeed::new(0x41d6_b9b2_4fb3_dd27),
    );
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
    let mut cx = Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        sim_kernel::HandleSeed::new(0xc73d_c9f6_f60f_a79d),
    );
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
fn symbolic_serial_series_round_trips_and_fails_closed() {
    let source = "#(SerialSeries alphabet_id=gesture/five-v1 symbols=[rise,fall,hold,turn,rest] rule=#(AggregateRule kind=ExhaustiveExactlyOnce) order=[turn,rise,rest,fall,hold])";
    let series = decode_serial_series(source).expect("serial series");
    let encoded = encode_serial_series(&series).expect("encode serial series");
    assert_eq!(
        decode_serial_series(&encoded)
            .unwrap_or_else(|error| panic!("round trip rejected {encoded}: {error}")),
        series
    );
    assert_eq!(series.permutation_rank().expect("rank").to_string(), "76");

    let foreign = source.replace("hold])", "unknown])");
    assert!(decode_serial_series(&foreign).is_err());
    let repeated = source.replace("rest,fall,hold", "rest,fall,fall");
    assert!(decode_serial_series(&repeated).is_err());
}

#[test]
fn custom_alphabet_recipe_executes_through_lisp_runtime_surface() {
    let mut cx = Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        sim_kernel::HandleSeed::new(0x24ef_2b7f_3135_a7c2),
    );
    install_music_shapes_lib(&mut cx).expect("music shapes");
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).expect("lisp codec");
    cx.load_lib(&lisp).expect("load lisp codec");

    let recipes =
        sim_cookbook::recipes_from_embedded(sim_lib_serial_core::RECIPES).expect("serial recipes");
    let recipe = recipes
        .iter()
        .find(|recipe| recipe.id.ends_with("/custom-alphabet"))
        .expect("custom alphabet recipe");
    let source = String::from_utf8(recipe.setup.clone()).expect("UTF-8 recipe");
    let expression = decode_eval_expr_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text(source),
        ReadPolicy {
            trust: TrustLevel::TrustedSource,
            capabilities: CapabilitySet::new(),
        },
    )
    .expect("decode recipe");
    let output = cx.eval_expr(expression).expect("evaluate recipe");
    let Expr::Map(fields) = output.object().as_expr(&mut cx).expect("result expression") else {
        panic!("serial validation must return a ledger map");
    };
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == &Expr::Symbol(Symbol::new("alphabet-id")))
            .map(|(_, value)| value),
        Some(&Expr::String("gesture/five-v1".to_owned()))
    );
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == &Expr::Symbol(Symbol::new("permutation-rank")))
            .map(|(_, value)| value),
        Some(&Expr::String("76".to_owned()))
    );

    let shape = registered_music_shape(&cx, "SerialSeries");
    let serial_source = decode_serial_call_source(&recipe.setup).expect("series source");
    assert_shape_accepts(&mut cx, &shape, &serial_source);
    assert_shape_rejects(
        &mut cx,
        &shape,
        &serial_source.replace("hold])", "unknown])"),
    );
}

#[test]
fn lisp_serial_realization_surface_round_trips_through_lisp_and_json() {
    let mut cx = Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        sim_kernel::HandleSeed::new(0x177e_81ca_3297_0b20),
    );
    install_music_shapes_lib(&mut cx).expect("music shapes");
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).expect("lisp codec");
    cx.load_lib(&lisp).expect("load lisp codec");
    let json = JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json).expect("load json codec");

    let request_expr = serial_realization_request_expr();
    let request_value =
        sim_citizen::value_from_expr(&mut cx, &request_expr).expect("request value");
    let request_lisp = encode_value_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        &request_value,
        EncodeOptions::default(),
    )
    .unwrap()
    .into_text()
    .unwrap();
    let expr = decode_eval_expr_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text(format!(
            "(serial/realize {request_lisp} :with \"modal-degree-cycle\")"
        )),
        ReadPolicy {
            trust: TrustLevel::TrustedSource,
            capabilities: CapabilitySet::new(),
        },
    )
    .unwrap();
    let realized = cx.eval_expr(expr).unwrap();
    let realized_expr = realized.object().as_expr(&mut cx).unwrap();
    let Expr::Map(fields) = &realized_expr else {
        panic!("serial/realize must return a realization map");
    };
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == &Expr::Symbol(Symbol::new("form")))
            .map(|(_, value)| value),
        Some(&Expr::String("SerialRealization".to_owned()))
    );
    assert_eq!(
        fields
            .iter()
            .find(|(key, _)| key == &Expr::Symbol(Symbol::new("realizer-id")))
            .map(|(_, value)| value),
        Some(&Expr::String("realizer/modal-degree-cycle".to_owned()))
    );

    for codec in ["lisp", "json"] {
        let encoded = encode_value_with_codec(
            &mut cx,
            &Symbol::qualified("codec", codec),
            &realized,
            EncodeOptions::default(),
        )
        .unwrap();
        let decoded = decode_with_codec(
            &mut cx,
            &Symbol::qualified("codec", codec),
            encoded.into_text().map(Input::Text).unwrap(),
            ReadPolicy {
                trust: TrustLevel::TrustedSource,
                capabilities: CapabilitySet::new(),
            },
        )
        .unwrap();
        assert_eq!(decoded, realized_expr);
    }
}

fn decode_serial_call_source(source: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(source).ok()?;
    let start = text.find('"')? + 1;
    let end = text.rfind('"')?;
    (start <= end).then(|| text[start..end].to_owned())
}

#[path = "tests_read_construct.rs"]
mod tests_read_construct;

use tests_read_construct::{assert_shape_accepts, assert_shape_rejects, registered_music_shape};
