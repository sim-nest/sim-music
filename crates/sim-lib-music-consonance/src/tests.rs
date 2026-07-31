use std::sync::Arc;

// conformance: exact score consonance retains identity, multiplicity, separate metrics, and MIDI parity.

use num_rational::Ratio;
use sim_kernel::{DefaultFactory, EagerPolicy, Expr, QuoteMode, RawArgs, Symbol};
use sim_lib_midi_core::{
    Channel, ChannelMessage, MetaEvent, MidiEvent, MidiPayload, TickTime, U7, synthetic_origin,
};
use sim_lib_midi_smf::{SmfDivision, SmfFile, SmfFormat, SmfTrack};
use sim_lib_music_core::{
    Articulation, Chord, Music, Note, ObjectId, Pitch, Score, Staff, StaffNote, StaffVoice, Time,
};
use sim_lib_music_lift::{MidiRealizationPolicy, realize_midi};

use crate::{
    ConsonancePolicy, TimeSpan, evaluate, evaluate_midi_timeline, evaluate_staff,
    install_music_consonance_lib, music_consonance_evaluate_symbol, slice_sounding_windows,
    sounding_windows,
};

fn time(numerator: i64, denominator: i64) -> Time {
    Ratio::new(numerator, denominator)
}

fn object_id(value: &str) -> ObjectId {
    ObjectId::new(value).expect("identity")
}

fn channel() -> Channel {
    Channel::new(0).expect("channel")
}

fn staff_note(
    voice: &str,
    id: &str,
    midi: u8,
    onset: Time,
    duration: Time,
    velocity: u8,
    articulation: Articulation,
) -> StaffNote {
    StaffNote {
        voice_id: object_id(voice),
        note_id: object_id(&format!("note/{id}")),
        event_id: object_id(&format!("event/{id}")),
        onset,
        note: Note::new(
            duration,
            Pitch::from_midi(midi),
            velocity,
            channel(),
            articulation,
        )
        .expect("note"),
    }
}

fn staff(duration: Time, notes: Vec<StaffNote>) -> Staff {
    Staff::new(vec![StaffVoice {
        id: object_id("voice/main"),
        name: "Main".to_owned(),
        duration,
        notes,
    }])
    .expect("staff")
}

#[test]
fn exact_windows_are_half_open_and_retain_duplicate_note_facts() {
    let score = staff(
        time(1, 1),
        vec![
            staff_note(
                "voice/main",
                "first",
                60,
                time(0, 1),
                time(1, 2),
                90,
                Articulation::Staccato,
            ),
            staff_note(
                "voice/main",
                "duplicate",
                60,
                time(1, 4),
                time(1, 2),
                110,
                Articulation::Legato,
            ),
        ],
    );
    let windows = sounding_windows(&score).expect("windows");

    assert_eq!(
        windows
            .iter()
            .map(|window| (window.span.start, window.span.end, window.notes.len()))
            .collect::<Vec<_>>(),
        vec![
            (time(0, 1), time(1, 4), 1),
            (time(1, 4), time(1, 2), 2),
            (time(1, 2), time(3, 4), 1),
            (time(3, 4), time(1, 1), 0),
        ]
    );
    let duplicate_window = &windows[1];
    assert_eq!(
        duplicate_window.notes[0].pitch,
        duplicate_window.notes[1].pitch
    );
    assert_ne!(
        duplicate_window.notes[0].event_id,
        duplicate_window.notes[1].event_id
    );
    assert_eq!(duplicate_window.notes[0].velocity, 90);
    assert_eq!(duplicate_window.notes[1].articulation, Articulation::Legato);
    assert_eq!(windows[2].notes[0].onset, time(1, 4));
    assert_eq!(windows[2].notes[0].release, time(3, 4));
}

#[test]
fn exact_window_slicing_is_associative_and_keeps_source_lifetimes() {
    let score = staff(
        time(1, 1),
        vec![staff_note(
            "voice/main",
            "held",
            67,
            time(0, 1),
            time(1, 1),
            100,
            Articulation::Tenuto,
        )],
    );
    let windows = sounding_windows(&score).expect("windows");
    let outer = TimeSpan::new(time(1, 8), time(7, 8)).expect("span");
    let inner = TimeSpan::new(time(1, 4), time(3, 4)).expect("span");
    let twice = slice_sounding_windows(&slice_sounding_windows(&windows, outer), inner.clone());
    let once = slice_sounding_windows(&windows, inner);

    assert_eq!(twice, once);
    assert_eq!(once[0].notes[0].onset, time(0, 1));
    assert_eq!(once[0].notes[0].release, time(1, 1));
}

#[test]
fn silence_is_an_exact_window_with_zero_separate_metrics() {
    let report = evaluate_staff(&staff(time(1, 2), Vec::new()), &ConsonancePolicy::default())
        .expect("silence");

    assert_eq!(report.windows.len(), 1);
    let silence = &report.windows[0];
    assert!(silence.window.notes.is_empty());
    assert!(silence.pitch.iter().all(zero_metric));
    assert!(silence.acoustic.iter().all(zero_metric));
    assert!(zero_metric(&silence.ratio));
    assert!(zero_metric(&silence.commonality));
    assert!(zero_metric(&silence.leading));
}

#[test]
fn every_metric_family_is_separate_and_density_is_not_mass() {
    let report = evaluate_staff(
        &staff(
            time(1, 2),
            vec![
                staff_note(
                    "voice/main",
                    "c",
                    60,
                    time(0, 1),
                    time(1, 2),
                    127,
                    Articulation::Normal,
                ),
                staff_note(
                    "voice/main",
                    "cs",
                    61,
                    time(0, 1),
                    time(1, 2),
                    96,
                    Articulation::Normal,
                ),
                staff_note(
                    "voice/main",
                    "g",
                    67,
                    time(0, 1),
                    time(1, 2),
                    80,
                    Articulation::Normal,
                ),
            ],
        ),
        &ConsonancePolicy::default(),
    )
    .expect("evaluation");
    let window = &report.windows[0];

    assert_eq!(window.pitch.len(), 4);
    assert_eq!(window.acoustic.len(), 4);
    assert_eq!(window.ratio.model, "ratio");
    assert_eq!(window.commonality.model, "commonality");
    assert_eq!(window.leading.model, "leading");
    let roughness = window
        .acoustic
        .iter()
        .find(|metric| metric.model == "plomp-levelt")
        .expect("roughness");
    assert!(roughness.roughness_mass > 0.0);
    assert!(roughness.normalized_density > 0.0);
    assert_ne!(
        roughness.roughness_mass, roughness.normalized_density,
        "mass and density must remain different named components"
    );
}

#[test]
fn octave_move_preserves_pitch_models_but_is_visible_to_leading() {
    let report = evaluate_staff(
        &staff(
            time(1, 1),
            vec![
                staff_note(
                    "voice/main",
                    "low",
                    60,
                    time(0, 1),
                    time(1, 2),
                    100,
                    Articulation::Normal,
                ),
                staff_note(
                    "voice/main",
                    "high",
                    72,
                    time(1, 2),
                    time(1, 2),
                    100,
                    Articulation::Normal,
                ),
            ],
        ),
        &ConsonancePolicy::default(),
    )
    .expect("evaluation");

    assert_eq!(report.windows[0].pitch, report.windows[1].pitch);
    assert_eq!(report.windows[1].leading.roughness_mass, 12.0);
    assert_ne!(
        report.windows[0].acoustic, report.windows[1].acoustic,
        "acoustic frequency remains octave-aware"
    );
}

#[test]
fn score_and_realized_midi_have_equivalent_windows_and_metrics() {
    let pitches = vec![
        Pitch::from_midi(60),
        Pitch::from_midi(64),
        Pitch::from_midi(67),
    ];
    let score = Score::new(
        120,
        (4, 4),
        None,
        Music::Chord(Chord::new(time(1, 4), "C", pitches, 100, channel()).expect("chord")),
    )
    .expect("score");
    let file = single_track(vec![
        note_on(0, 60),
        note_on(0, 64),
        note_on(0, 67),
        note_off(480, 60),
        note_off(480, 64),
        note_off(480, 67),
        event(480, MidiPayload::Meta(MetaEvent::EndOfTrack)),
    ]);
    let midi = realize_midi(&file, MidiRealizationPolicy::default()).expect("MIDI realization");
    let score_report = evaluate(&score, &ConsonancePolicy::default()).expect("score evaluation");
    let midi_report = evaluate_midi_timeline(&midi.timelines[0], &ConsonancePolicy::default())
        .expect("MIDI evaluation");

    assert_eq!(score_report.windows.len(), midi_report.windows.len());
    assert_eq!(
        score_report.windows[0].window.span,
        midi_report.windows[0].window.span
    );
    assert_eq!(score_report.windows[0].pitch, midi_report.windows[0].pitch);
    assert_eq!(
        score_report.windows[0].acoustic,
        midi_report.windows[0].acoustic
    );
    assert_eq!(score_report.windows[0].ratio, midi_report.windows[0].ratio);
}

#[test]
fn runtime_report_exposes_notes_and_each_metric_without_an_aggregate() {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_consonance_lib(&mut cx).expect("install");
    let function = cx
        .resolve_function(&music_consonance_evaluate_symbol())
        .expect("function");
    let callable = function.object().as_callable().expect("callable");
    let args = vec![
        Expr::Symbol(Symbol::new(":score")),
        Expr::String(
            "#(Score tempo=120 time_sig=4/4 key=none body=#(Chord dur=1/4 symbol=\"C\" pitches=[C4,E4,G4] vel=100 channel=0))"
                .to_owned(),
        ),
        Expr::Symbol(Symbol::new(":policy")),
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new(":duplicates")),
                quoted_symbol("retain"),
            ),
            (
                Expr::Symbol(Symbol::new(":normalization")),
                quoted_symbol("per-pair"),
            ),
        ]),
    ];
    let value = callable
        .call_exprs(&mut cx, RawArgs::new(args))
        .expect("runtime evaluation");
    let Expr::Map(report) = value.object().as_expr(&mut cx).expect("expression") else {
        panic!("expected report map");
    };

    assert!(has_key(&report, "provenance"));
    assert!(has_key(&report, "windows"));
    assert_eq!(field(&report, "aggregate"), Some(&Expr::Nil));
    let Some(Expr::Vector(windows)) = field(&report, "windows") else {
        panic!("expected windows vector");
    };
    let Expr::Map(window) = &windows[0] else {
        panic!("expected window map");
    };
    for key in [
        "notes",
        "pitch",
        "acoustic",
        "ratio",
        "commonality",
        "leading",
    ] {
        assert!(has_key(window, key), "missing {key}");
    }
}

fn zero_metric(metric: &crate::MetricReport) -> bool {
    metric.roughness_mass == 0.0
        && metric.normalized_density == 0.0
        && metric.harmonic_context == 0.0
}

fn event(ticks: i64, payload: MidiPayload) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, 480).expect("tick"),
        origin: synthetic_origin(),
        payload,
    }
}

fn note_on(ticks: i64, key: u8) -> MidiEvent {
    event(
        ticks,
        MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: channel(),
            key: U7(key),
            vel: U7(100),
        }),
    )
}

fn note_off(ticks: i64, key: u8) -> MidiEvent {
    event(
        ticks,
        MidiPayload::Channel(ChannelMessage::NoteOff {
            ch: channel(),
            key: U7(key),
            vel: U7(0),
        }),
    )
}

fn single_track(events: Vec<MidiEvent>) -> SmfFile {
    SmfFile {
        format: SmfFormat::SingleTrack,
        division: SmfDivision::metrical(480).expect("division"),
        tracks: vec![SmfTrack { events }],
    }
}

fn quoted_symbol(value: &str) -> Expr {
    Expr::Quote {
        mode: QuoteMode::Quote,
        expr: Box::new(Expr::Symbol(Symbol::new(value))),
    }
}

fn has_key(entries: &[(Expr, Expr)], name: &str) -> bool {
    field(entries, name).is_some()
}

fn field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
        _ => None,
    })
}
