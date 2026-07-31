use std::sync::Arc;

use sim_codec::{Input, decode_with_codec};
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, ReadPolicy, Symbol};

// conformance: catalog synthesis reuse composes checked owners without copied DSP.

#[test]
fn catalog_reuse_composition_recipe_is_bounded_and_non_destructive() {
    let source = include_str!("../../recipes/03-composition/offline-realtime-preview/setup.siml");
    let plan = decode_plan(source);
    let catalog = as_vector(field(&plan, "catalog"));
    assert_eq!(catalog.len(), 9);
    for row in catalog {
        let row = as_map(Some(row));
        assert!(field(row, "need").is_some());
        assert!(field(row, "owner").is_some());
        assert!(matches!(field(row, "specimen"), Some(Expr::String(_))));
    }

    let input = as_map(field(&plan, "input"));
    assert_eq!(symbol_text(field(input, "kind")), "table/dir");
    assert_eq!(string_text(field(input, "dir-handle")), "music-inputs");
    assert_eq!(string_text(field(input, "value-handle")), "fm-bell-notes");

    let offline = as_map(field(&plan, "offline"));
    assert_eq!(symbol_text(field(offline, "owner")), "sim-lib-sound-render");
    assert_eq!(number_text(field(offline, "sample-rate")), "48000");
    assert_non_destructive_output(as_map(field(offline, "output")));

    let realtime = as_map(field(&plan, "realtime"));
    assert_eq!(
        symbol_text(field(realtime, "runner-owner")),
        "sim-lib-audio-graph-live"
    );
    let effects = as_vector(field(realtime, "effects"));
    assert_eq!(effects.len(), 2);
    assert_eq!(symbol_text(Some(&effects[0])), "audio-dsp/BiquadFilter");
    assert_eq!(symbol_text(Some(&effects[1])), "audio-dsp/Limiter");
    let callback = as_map(field(realtime, "callback"));
    assert_eq!(field(callback, "preallocated"), Some(&Expr::Bool(true)));
    assert_eq!(field(callback, "locks"), Some(&Expr::Bool(false)));
    assert_eq!(field(callback, "io"), Some(&Expr::Bool(false)));
    assert_non_destructive_output(as_map(field(realtime, "preview-output")));

    let wav = as_map(field(&plan, "wav"));
    assert_eq!(symbol_text(field(wav, "owner")), "sim-lib-stream-file");
    assert_eq!(symbol_text(field(wav, "input-format")), "pcm16");
    assert_non_destructive_output(as_map(field(wav, "output")));
    let deferred = as_map(field(wav, "deferred"));
    for policy in ["sample-conversion", "dither"] {
        assert_eq!(string_text(field(deferred, policy)), "MUSICALGOS4.43");
    }

    assert!(!source.contains(":path"));
    assert!(!source.contains("shell"));
    assert!(!source.contains("exec"));
}

fn decode_plan(source: &str) -> Vec<(Expr, Expr)> {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    sim_test_support::register_f64_number_domain(&mut cx);
    let codec = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).expect("lisp codec");
    cx.load_lib(&codec).expect("load lisp codec");
    let decoded = decode_with_codec(
        &mut cx,
        &Symbol::qualified("codec", "lisp"),
        Input::Text(source.trim().to_owned()),
        ReadPolicy::default(),
    )
    .expect("decode composition recipe");
    let Expr::Quote { expr, .. } = decoded else {
        panic!("composition recipe must be quoted data");
    };
    let Expr::Map(plan) = *expr else {
        panic!("composition recipe must contain a map");
    };
    plan
}

fn assert_non_destructive_output(output: &[(Expr, Expr)]) {
    assert!(matches!(field(output, "dir-handle"), Some(Expr::String(_))));
    assert!(matches!(
        field(output, "value-handle"),
        Some(Expr::String(_))
    ));
    assert_eq!(field(output, "replace"), Some(&Expr::Bool(false)));
}

fn field<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
        _ => None,
    })
}

fn as_map(value: Option<&Expr>) -> &[(Expr, Expr)] {
    let Some(Expr::Map(entries)) = value else {
        panic!("expected map, got {value:?}");
    };
    entries
}

fn as_vector(value: Option<&Expr>) -> &[Expr] {
    let Some(Expr::Vector(items)) = value else {
        panic!("expected vector, got {value:?}");
    };
    items
}

fn symbol_text(value: Option<&Expr>) -> String {
    let Some(Expr::Symbol(symbol)) = value else {
        panic!("expected symbol, got {value:?}");
    };
    symbol.as_qualified_str()
}

fn string_text(value: Option<&Expr>) -> &str {
    let Some(Expr::String(value)) = value else {
        panic!("expected string, got {value:?}");
    };
    value
}

fn number_text(value: Option<&Expr>) -> String {
    let Some(Expr::Number(value)) = value else {
        panic!("expected number, got {value:?}");
    };
    value.canonical.clone()
}
