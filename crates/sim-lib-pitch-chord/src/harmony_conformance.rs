//! Checked conformance for the declarative catalog's Lisp data fixtures.

use std::sync::Arc;

use sim_codec::{Input, decode_with_codec};
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, ReadPolicy, Symbol};

use crate::{HarmonyMetric, HarmonyPredicate, HarmonyProgram, PaletteAlgebra};

fn decode_fixture(source: &str) -> HarmonyProgram {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    sim_test_support::register_f64_number_domain(&mut cx);
    let codec = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&codec).unwrap();
    let expr = decode_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text(source.to_owned()),
        ReadPolicy::default(),
    )
    .unwrap();
    HarmonyProgram::from_expr(&expr).unwrap()
}

#[test]
fn catalog_harmonizer_lisp_fixture_covers_every_filter_and_render_setting() {
    let program = decode_fixture(include_str!("../tests/fixtures/catalog-harmonizer.sx"));
    let ids = program
        .rules
        .hard
        .iter()
        .map(|rule| rule.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(program.id, "catalog/harmonizer");
    assert_eq!(ids.len(), 23);
    for required in [
        "melody-in-chord",
        "position-is",
        "not-position-is",
        "not-position-is-not",
        "common-note-rhythm",
        "minimum-chord-distance",
        "maximum-chord-distance",
        "minimum-type-distance",
        "chord-variation",
        "chord-commonality",
        "minimum-scale-duration",
        "maximum-scale-duration",
        "template-length",
        "will-connect",
        "template-melody-in-chord",
    ] {
        assert!(ids.contains(&required), "{required}");
    }
    assert!(program.rules.hard.iter().any(|rule| {
        matches!(
            rule.predicate,
            HarmonyPredicate::All(_) | HarmonyPredicate::Any(_) | HarmonyPredicate::Not(_)
        )
    }));
    assert!(
        program
            .rules
            .soft
            .iter()
            .any(|metric| { matches!(metric.value, HarmonyMetric::PitchDissonance { .. }) })
    );
    assert!(
        program
            .rules
            .soft
            .iter()
            .any(|metric| { matches!(metric.value, HarmonyMetric::ContextualSonance { .. }) })
    );
    assert_eq!(program.render.chord_transpose, 48);
    assert_eq!(program.render.melody_transpose, 60);
    assert_eq!(program.render.duration_multiplier, 4);
    assert_eq!(program.render.tempo_bpm, 60);
    assert_eq!(program.render.time_signature, (4, 4));
}

#[test]
fn catalog_chord_cycle_lisp_fixture_names_transposed_palette_and_filter_chain() {
    let program = decode_fixture(include_str!("../tests/fixtures/catalog-chord-cycle.sx"));

    assert_eq!(program.id, "catalog/chord-cycle");
    assert!(matches!(
        program.palette.algebra,
        PaletteAlgebra::Transpose { .. }
    ));
    assert_eq!(
        program
            .rules
            .hard
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "constant-common-notes",
            "minimum-type-distance",
            "required-first",
            "final-chord-only",
        ]
    );
}
