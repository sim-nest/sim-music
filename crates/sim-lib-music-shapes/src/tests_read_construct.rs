use super::*;

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
    let mut cx = Cx::new(
        Arc::new(EagerPolicy),
        Arc::new(DefaultFactory),
        sim_kernel::HandleSeed::new(0xecda_159d_eeeb_b60c),
    );
    cx.load_lib(&sim_citizen::CitizenLib::all()).unwrap();
    cx.grant(read_construct_capability());
    cx
}

pub(super) fn registered_music_shape(cx: &Cx, name: &'static str) -> Value {
    cx.registry()
        .shape_by_symbol(&Symbol::qualified("music", name))
        .expect("registered music shape")
        .clone()
}

pub(super) fn assert_shape_accepts(cx: &mut Cx, shape: &Value, text: &str) {
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

pub(super) fn assert_shape_rejects(cx: &mut Cx, shape: &Value, text: &str) {
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

    let realization_policy = MidiRealizationPolicy {
        overlap: OverlapPolicy::Lifo,
        same_tick: SameTickPolicy::NoteOnsFirst,
        dangling_notes: DanglingNotePolicy::Reject,
        pedals: PedalPolicy::Sustain,
    };
    assert_eq!(
        decode_midi_realization_policy(&encode_midi_realization_policy(&realization_policy))
            .expect("realization policy"),
        realization_policy
    );
}
