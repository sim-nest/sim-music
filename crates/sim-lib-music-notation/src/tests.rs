use num_rational::Ratio;
use std::sync::Arc;

use sim_kernel::{DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_music_core::{
    Articulation, Channel, Chord, Melody, MelodyItem, Music, Note, Progression, Score,
};
use sim_lib_pitch_core::Pitch;

use crate::{
    NotationCodec, NotationError, export_counterpoint_lilypond, export_lilypond,
    export_melody_lilypond, export_progression_lilypond, import_lilypond,
    install_music_notation_lib,
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
}
