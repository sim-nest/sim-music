//! LilyPond and bounded MusicXML notation profile conformance: exact exchange,
//! strict rejection, stable identity, explicit loss, and runtime construction.

use num_rational::Ratio;
use std::sync::Arc;

use sim_kernel::{DefaultFactory, EagerPolicy, Expr, QuoteMode, Symbol};
use sim_lib_music_core::{
    Articulation, Channel, Chord, Melody, MelodyItem, Music, Note, Progression, Score,
};
use sim_lib_music_shapes::{decode_score, encode_score};
use sim_lib_pitch_core::Pitch;

use crate::{
    MusicXmlLimits, NotationCodec, NotationError, NotationIdentityKind, NotationLossKind,
    export_counterpoint_lilypond, export_lilypond, export_melody_lilypond,
    export_musicxml_partwise_report, export_progression_lilypond, import_lilypond,
    import_musicxml_partwise_report, install_music_notation_lib, notation_import_symbol,
};

fn quarter() -> Ratio<i64> {
    Ratio::new(1, 4)
}

fn channel() -> Channel {
    Channel::new(0).expect("channel")
}

fn note(midi: u8, duration: Ratio<i64>) -> Note {
    Note::new(
        duration,
        Pitch::from_midi(midi),
        100,
        channel(),
        Articulation::Normal,
    )
    .expect("note")
}

fn exchange_score() -> Score {
    let melody = Melody::new(vec![
        MelodyItem::Note(note(60, quarter())),
        MelodyItem::Rest(sim_lib_music_core::Rest::new(quarter()).expect("rest")),
        MelodyItem::Note(note(64, Ratio::new(1, 2))),
    ])
    .expect("melody");
    Score::new(
        120,
        (4, 4),
        Some("C major".to_owned()),
        Music::Melody(melody),
    )
    .expect("score")
}

fn canonical_score(score: &Score) -> String {
    encode_score(score).expect("canonical score")
}

#[test]
fn simple_melody_exports_valid_lilypond_text() {
    let melody = Melody::new(vec![
        MelodyItem::Note(note(60, quarter())),
        MelodyItem::Rest(sim_lib_music_core::Rest::new(quarter()).expect("rest")),
        MelodyItem::Note(note(62, Ratio::new(3, 8))),
    ])
    .expect("melody");
    let score = Score::new(
        120,
        (4, 4),
        Some("C major".to_owned()),
        Music::Melody(melody),
    )
    .expect("score");
    let text = export_lilypond(&score).expect("export");
    assert!(text.contains("\\score"));
    assert!(text.contains("\\tempo 4 = 120"));
    assert!(text.contains("\\key c \\major"));
    assert!(text.contains("c'4"));
    assert!(text.contains("r4"));
    assert!(text.contains("d'4 ~ d'8"));
}

#[test]
fn four_voice_counterpoint_exports_four_voices_with_names() {
    let voice = Melody::new(vec![MelodyItem::Note(note(60, quarter()))]).expect("melody");
    let counterpoint = sim_lib_music_core::Counterpoint::new(
        vec![voice.clone(), voice.clone(), voice.clone(), voice],
        vec![
            "Soprano".to_owned(),
            "Alto".to_owned(),
            "Tenor".to_owned(),
            "Bass".to_owned(),
        ],
    )
    .expect("counterpoint");
    let text = export_counterpoint_lilypond(&counterpoint, Some("C major")).expect("export");
    assert!(text.contains("\\new Voice = \"Soprano\""));
    assert!(text.contains("\\new Voice = \"Bass\""));
}

#[test]
fn export_then_import_preserves_supported_subset() {
    let progression = Progression::new(
        Some("C major".to_owned()),
        vec![
            Chord::new(
                quarter(),
                "C",
                vec![
                    Pitch::from_midi(60),
                    Pitch::from_midi(64),
                    Pitch::from_midi(67),
                ],
                100,
                channel(),
            )
            .expect("chord"),
            Chord::new(
                Ratio::new(1, 2),
                "F",
                vec![
                    Pitch::from_midi(65),
                    Pitch::from_midi(69),
                    Pitch::from_midi(72),
                ],
                100,
                channel(),
            )
            .expect("chord"),
        ],
    )
    .expect("progression");
    let score = Score::new(
        96,
        (3, 4),
        Some("C major".to_owned()),
        Music::Progression(progression.clone()),
    )
    .expect("score");
    let lily = export_lilypond(&score).expect("export");
    let imported = import_lilypond(&lily).expect("import");
    assert_eq!(imported.tempo_bpm, score.tempo_bpm);
    assert_eq!(imported.time_signature, score.time_signature);
    assert_eq!(imported.key, score.key);
    match imported.body {
        Music::Progression(value) => {
            assert_eq!(value.chords.len(), progression.chords.len());
            assert_eq!(value.chords[0].pitches, progression.chords[0].pitches);
            assert_eq!(value.chords[1].duration, progression.chords[1].duration);
        }
        other => panic!("expected progression, got {other:?}"),
    }
}

#[test]
fn enharmonic_spelling_survives_within_same_key() {
    let melody = Melody::new(vec![MelodyItem::Note(note(66, quarter()))]).expect("melody");
    let exported = export_melody_lilypond(&melody, Some("G major")).expect("export");
    assert!(exported.contains("fis'4"));
    let score =
        format!("\\score {{\n  \\tempo 4 = 100\n  \\key g \\major\n  \\time 4/4\n  {exported}\n}}");
    let imported = import_lilypond(&score).expect("import");
    let reexported = NotationCodec.export_lilypond(&imported).expect("re-export");
    assert!(reexported.contains("fis'4"));
}

#[test]
fn unsupported_syntax_reports_diagnostic_not_panic() {
    let err = import_lilypond("\\score { \\relative c' { c4 } }").expect_err("unsupported");
    match err {
        NotationError::UnsupportedSyntax { diagnostics } => {
            assert!(!diagnostics.is_empty());
            assert!(diagnostics[0].message.contains("unsupported"));
            assert!(diagnostics[0].span.is_some());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn progression_export_helper_uses_chord_bodies() {
    let progression = Progression::new(
        Some("Bb major".to_owned()),
        vec![
            Chord::new(
                quarter(),
                "Bb",
                vec![
                    Pitch::from_midi(58),
                    Pitch::from_midi(62),
                    Pitch::from_midi(65),
                ],
                100,
                channel(),
            )
            .expect("chord"),
        ],
    )
    .expect("progression");
    let text = export_progression_lilypond(&progression, Some("Bb major")).expect("export");
    assert!(text.contains("<bes d' f'>4"));
}

#[test]
fn install_music_notation_lib_registers_codec_surface() {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_notation_lib(&mut cx).expect("install");
    install_music_notation_lib(&mut cx).expect("install");
    let value = cx
        .resolve_value(&Symbol::qualified("music", "LilyPondSubsetCodec"))
        .expect("value");
    let expr = value.object().as_expr(&mut cx).expect("expr");
    let Expr::Map(entries) = expr else {
        panic!("expected browse table");
    };
    assert!(entries.iter().any(|(key, value)| {
        *key == Expr::Symbol(Symbol::new("shape"))
            && *value == Expr::Symbol(Symbol::qualified("music", "NotationCodec"))
    }));
    assert!(cx.resolve_function(&notation_import_symbol()).is_ok());
}

#[test]
fn musicxml_partwise_round_trip_retains_exact_score_and_stable_ids() {
    let score = exchange_score();
    let exported = export_musicxml_partwise_report(&score, &[]).expect("export");
    assert!(exported.losses.is_empty());
    assert_eq!(
        exported
            .identities
            .iter()
            .filter(|identity| identity.kind == NotationIdentityKind::Event)
            .count(),
        3
    );

    let source = exported
        .value
        .replace("P1-E1", "lead-note")
        .replace("P1-E2", "lead-rest")
        .replace("P1-E3", "lead-final")
        .replace("id=\"P1\"", "id=\"lead-part\"");
    let imported = import_musicxml_partwise_report(source.as_bytes(), MusicXmlLimits::default())
        .expect("import");
    assert_eq!(canonical_score(&imported.value), canonical_score(&score));
    assert!(imported.losses.is_empty());
    assert!(
        imported
            .identities
            .iter()
            .any(|identity| identity.xml_id == "lead-note")
    );

    let reexported =
        export_musicxml_partwise_report(&imported.value, &imported.identities).expect("re-export");
    assert!(reexported.value.contains("id=\"lead-part\""));
    assert!(reexported.value.contains("id=\"lead-note\""));
    assert!(reexported.value.contains("id=\"lead-rest\""));
    assert!(reexported.value.contains("id=\"lead-final\""));

    let mut wrong_kind = imported.identities;
    wrong_kind[0].kind = NotationIdentityKind::Event;
    assert!(export_musicxml_partwise_report(&imported.value, &wrong_kind).is_err());
}

#[test]
fn musicxml_profile_rejects_dtd_entities_extensions_and_resource_overruns() {
    let internal_dtd = br#"<?xml version="1.0"?>
<!DOCTYPE score-partwise [<!ENTITY boom "expanded">]>
<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>&boom;</part-name></score-part></part-list><part id="P1"><measure number="1"/></part></score-partwise>"#;
    let empty_dtd = br#"<!DOCTYPE score-partwise>
<score-partwise version="4.0"/>"#;
    for dtd in [internal_dtd.as_slice(), empty_dtd.as_slice()] {
        assert!(matches!(
            import_musicxml_partwise_report(dtd, MusicXmlLimits::default()),
            Err(NotationError::UnsupportedMusicXml { diagnostics })
                if diagnostics[0].message.contains("DTD")
        ));
    }

    let score = exchange_score();
    let source = export_musicxml_partwise_report(&score, &[])
        .expect("export")
        .value;
    let extension = source.replace("<note id=\"P1-E1\">", "<harmony/><note id=\"P1-E1\">");
    assert!(matches!(
        import_musicxml_partwise_report(extension.as_bytes(), MusicXmlLimits::default()),
        Err(NotationError::UnsupportedMusicXml { diagnostics })
            if diagnostics[0].message.contains("<harmony>")
    ));
    let processing_instruction = source.replace(
        "<note id=\"P1-E1\">",
        "<?sim extension?><note id=\"P1-E1\">",
    );
    assert!(matches!(
        import_musicxml_partwise_report(
            processing_instruction.as_bytes(),
            MusicXmlLimits::default()
        ),
        Err(NotationError::UnsupportedMusicXml { diagnostics })
            if diagnostics[0].message.contains("processing instruction")
    ));
    let mixed_text = source.replace("<part id=\"P1\">", "<part id=\"P1\">extension");
    assert!(matches!(
        import_musicxml_partwise_report(mixed_text.as_bytes(), MusicXmlLimits::default()),
        Err(NotationError::UnsupportedMusicXml { diagnostics })
            if diagnostics[0].message.contains("mixed text")
    ));
    let duplicate_pitch_step = source.replace("<step>C</step>", "<step>C</step><step>C</step>");
    assert!(matches!(
        import_musicxml_partwise_report(
            duplicate_pitch_step.as_bytes(),
            MusicXmlLimits::default()
        ),
        Err(NotationError::UnsupportedMusicXml { diagnostics })
            if diagnostics[0].message.contains("exactly one <step>")
    ));

    let limits = MusicXmlLimits {
        bytes: source.len() - 1,
        ..MusicXmlLimits::default()
    };
    assert!(matches!(
        import_musicxml_partwise_report(source.as_bytes(), limits),
        Err(NotationError::MusicXmlLimit { limit: "bytes", .. })
    ));
    let limits = MusicXmlLimits {
        depth: 3,
        ..MusicXmlLimits::default()
    };
    assert!(matches!(
        import_musicxml_partwise_report(source.as_bytes(), limits),
        Err(NotationError::MusicXmlLimit { limit: "depth", .. })
    ));
    let limits = MusicXmlLimits {
        events: 2,
        ..MusicXmlLimits::default()
    };
    assert!(import_musicxml_partwise_report(source.as_bytes(), limits).is_err());
}

#[test]
fn musicxml_reports_accepted_import_metadata_loss() {
    let source = export_musicxml_partwise_report(&exchange_score(), &[])
        .expect("export")
        .value
        .replace(
            "<part-name>Music</part-name>",
            "<part-name>Solo</part-name>",
        )
        .replace(
            "<pitch><step>C</step><octave>4</octave></pitch>",
            "<pitch><step>B</step><alter>1</alter><octave>3</octave></pitch>",
        );
    let report = import_musicxml_partwise_report(source.as_bytes(), MusicXmlLimits::default())
        .expect("import");
    assert!(
        report
            .losses
            .iter()
            .any(|loss| loss.kind == NotationLossKind::PartName)
    );
    assert!(
        report
            .losses
            .iter()
            .any(|loss| loss.kind == NotationLossKind::PitchSpelling)
    );
    assert_eq!(report.diagnostics.len(), report.losses.len());
}

#[test]
fn musicxml_reports_every_accepted_export_loss() {
    let lossy_note = Note::new(
        Ratio::new(1, 1),
        Pitch::from_midi(60),
        73,
        Channel::new(4).expect("channel"),
        Articulation::Normal,
    )
    .expect("note");
    let score = Score::new(90, (4, 4), None, Music::Note(lossy_note)).expect("score");
    let report = export_musicxml_partwise_report(&score, &[]).expect("export");
    assert_eq!(report.losses.len(), 2);
    assert!(
        report
            .losses
            .iter()
            .any(|loss| loss.kind == NotationLossKind::Velocity)
    );
    assert!(
        report
            .losses
            .iter()
            .any(|loss| loss.kind == NotationLossKind::Channel)
    );
    assert_eq!(report.diagnostics.len(), report.losses.len());
}

#[test]
fn lilypond_musicxml_and_sim_forms_round_trip_on_their_shared_domain() {
    let score = exchange_score();
    let lily = export_lilypond(&score).expect("LilyPond export");
    let from_lily = import_lilypond(&lily).expect("LilyPond import");
    let xml = export_musicxml_partwise_report(&from_lily, &[]).expect("MusicXML export");
    let from_xml = import_musicxml_partwise_report(xml.value.as_bytes(), MusicXmlLimits::default())
        .expect("MusicXML import")
        .value;
    let sim_form = encode_score(&from_xml).expect("SIM expression encode");
    let from_sim = decode_score(&sim_form).expect("SIM expression decode");
    assert_eq!(canonical_score(&from_sim), canonical_score(&score));
}

#[test]
fn runtime_import_is_shape_described_and_returns_score_read_construct() {
    let score = exchange_score();
    let source = export_musicxml_partwise_report(&score, &[])
        .expect("export")
        .value
        .into_bytes();
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_music_notation_lib(&mut cx).expect("install");
    let function = cx
        .resolve_function(&notation_import_symbol())
        .expect("notation import function");
    let callable = function.object().as_callable().expect("callable");
    let args = vec![
        Expr::Symbol(Symbol::new(":format")),
        Expr::Quote {
            mode: QuoteMode::Quote,
            expr: Box::new(Expr::Symbol(Symbol::new("musicxml-partwise"))),
        },
        Expr::Symbol(Symbol::new(":source")),
        Expr::Bytes(source),
        Expr::Symbol(Symbol::new(":limits")),
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new(":bytes")),
                Expr::String("4000000".to_owned()),
            ),
            (
                Expr::Symbol(Symbol::new(":nodes")),
                Expr::String("200000".to_owned()),
            ),
            (
                Expr::Symbol(Symbol::new(":depth")),
                Expr::String("64".to_owned()),
            ),
        ]),
    ];
    let shape = callable
        .browse_args_shape(&mut cx)
        .expect("args shape")
        .expect("shape");
    assert!(
        shape
            .object()
            .as_shape()
            .expect("shape protocol")
            .check_expr(&mut cx, &Expr::List(args.clone()))
            .expect("shape check")
            .accepted
    );

    let value = callable
        .call_exprs(&mut cx, sim_kernel::RawArgs::new(args))
        .expect("runtime import");
    let Expr::Map(entries) = value.object().as_expr(&mut cx).expect("report expression") else {
        panic!("expected report map");
    };
    let score = entries
        .iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if symbol.name.as_ref() == "score" => Some(value),
            _ => None,
        })
        .expect("score field");
    assert!(matches!(
        score,
        Expr::Extension { tag, .. }
            if *tag == Symbol::qualified("citizen", "read-construct")
    ));
}
