use std::sync::Arc;

use sim_codec::{Input, Output, decode_with_codec, encode_with_codec};
use sim_codec_json::JsonCodecLib;
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, EncodeOptions, Expr, ReadPolicy, Symbol};
use sim_lib_topology::validate::validate_graph;

use crate::{
    ComponentPortMedia, InstrumentPatch, PatchRawView,
    ps3300::{
        PS3300_KEY_COUNT, PS3300_RECIPE_BOOK_PATH, PS3300_RECIPE_CHAPTER_PATH,
        PS3300_SECTION_COUNT, PS3300_SOURCE_PATH, PS3300_TOTAL_KEY_CELLS, Ps3300PinMatrixRoute,
        ps3300_default_pin_matrix_routes, ps3300_module_ids, ps3300_patch_model_names,
        ps3300_per_key_cell_patch, ps3300_scaffold_patch, ps3300_scaffold_patch_id,
        ps3300_section_polyphonic_arrays, ps3300_validate_pin_matrix_routes,
    },
};

#[test]
fn ps3300_scaffold_records_module_ids_paths_and_model_names() {
    assert_eq!(
        PS3300_SOURCE_PATH,
        "crates/sim-lib-music-synth/src/ps3300.rs"
    );
    assert_eq!(
        PS3300_RECIPE_BOOK_PATH,
        "crates/sim-lib-music-synth/recipes/ps3300/book.toml"
    );
    assert_eq!(
        PS3300_RECIPE_CHAPTER_PATH,
        "crates/sim-lib-music-synth/recipes/ps3300/chapter.toml"
    );

    let ids = ps3300_module_ids()
        .into_iter()
        .map(|id| id.as_qualified_str())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 10);
    assert!(
        ids.iter()
            .all(|id| id.starts_with("audio-synth/module/ps3-"))
    );
    assert!(ids.contains(&"audio-synth/module/ps3-section-generator".to_owned()));
    assert!(ids.contains(&"audio-synth/module/ps3-pin-matrix".to_owned()));
    assert!(ids.contains(&"audio-synth/module/ps3-per-key-cell".to_owned()));

    let names = ps3300_patch_model_names();
    assert!(names.contains(&"Ps3300ResonatorSettings"));
    assert!(names.contains(&"Ps3300PinMatrixRoute"));
    assert!(names.contains(&"PolyphonicArray"));
}

#[test]
fn ps3300_patch_represents_sections_resonator_matrix_keyboard_and_raw_view() {
    let patch = ps3300_scaffold_patch();
    assert_eq!(patch.name, ps3300_scaffold_patch_id());
    assert_eq!(section_count(&patch), PS3300_SECTION_COUNT);
    assert_eq!(patch.cords.len(), 14);
    assert!(module_kind(&patch, "keyboard", "ps3-keyboard-controller"));
    assert!(module_kind(&patch, "pin-matrix", "ps3-pin-matrix"));
    assert!(module_kind(&patch, "resonator", "ps3-resonator-bank"));
    assert!(module_kind(&patch, "output-mixer", "ps3-output-mixer"));
    assert!(module_kind(&patch, "section-a", "ps3-section-generator"));
    assert!(module_kind(&patch, "section-b", "ps3-section-generator"));
    assert!(module_kind(&patch, "section-c", "ps3-section-generator"));

    let raw_view = patch.raw_view.as_ref().expect("raw view");
    assert_eq!(
        raw_view.format.as_qualified_str(),
        "audio-synth/raw/korg-ps-3300"
    );
    assert!(raw_field(raw_view, "keyboard-assignment").is_some());
    assert!(raw_field(raw_view, "resonator").is_some());
    assert!(raw_field(raw_view, "pin-matrix-routes").is_some());
}

#[test]
fn ps3300_uses_polyphonic_arrays_for_full_keyboard_cells() {
    let arrays = ps3300_section_polyphonic_arrays();
    assert_eq!(arrays.len(), PS3300_SECTION_COUNT);
    assert!(
        arrays
            .iter()
            .all(|array| array.voice_count() == PS3300_KEY_COUNT)
    );
    assert_eq!(
        arrays
            .iter()
            .map(|array| array.voice_count())
            .sum::<usize>(),
        PS3300_TOTAL_KEY_CELLS
    );

    let cells = ps3300_per_key_cell_patch();
    assert_eq!(cells.modules.len(), PS3300_TOTAL_KEY_CELLS);
    assert!(cells.modules.iter().all(|module| {
        module.kind.as_qualified_str() == "audio-synth/module/ps3-per-key-cell"
            && module
                .inputs
                .iter()
                .any(|jack| jack.media == ComponentPortMedia::ControlVoltage)
            && module
                .inputs
                .iter()
                .any(|jack| jack.media == ComponentPortMedia::Gate)
    }));
}

#[test]
fn ps3300_patch_round_trips_through_expr_lisp_json_and_topology() {
    let patch = ps3300_scaffold_patch();
    let expr = patch.to_expr();
    assert_eq!(
        InstrumentPatch::from_expr(&expr).expect("expr patch"),
        patch
    );

    for codec in ["lisp", "json"] {
        let decoded = codec_roundtrip(codec, &expr);
        let decoded_patch = InstrumentPatch::from_expr(&decoded).expect("decoded patch");
        assert_eq!(decoded_patch.name, patch.name, "{codec}");
        assert_eq!(decoded_patch.modules.len(), patch.modules.len(), "{codec}");
        assert_eq!(decoded_patch.cords.len(), patch.cords.len(), "{codec}");
        assert_eq!(
            section_count(&decoded_patch),
            PS3300_SECTION_COUNT,
            "{codec}"
        );
        let raw_view = decoded_patch.raw_view.as_ref().expect("decoded raw view");
        assert!(
            raw_field(raw_view, "keyboard-assignment").is_some(),
            "{codec}"
        );
        assert!(raw_field(raw_view, "resonator").is_some(), "{codec}");
        assert!(
            raw_field(raw_view, "pin-matrix-routes").is_some(),
            "{codec}"
        );
    }

    validate_graph(&mut topology_cx(), &patch.topology_graph()).expect("PS-3300 topology");
}

#[test]
fn ps3300_pin_matrix_rejects_invalid_routing() {
    ps3300_validate_pin_matrix_routes(&ps3300_default_pin_matrix_routes()).expect("default routes");

    let err = ps3300_validate_pin_matrix_routes(&[Ps3300PinMatrixRoute::new(
        "bad-source",
        "section-a-pitch-cv",
        1.0,
    )])
    .expect_err("bad source should fail");
    assert!(format!("{err}").contains("unknown PS-3300 pin source bad-source"));

    let err = ps3300_validate_pin_matrix_routes(&[Ps3300PinMatrixRoute::new(
        "keyboard-pitch-cv",
        "bad-target",
        1.0,
    )])
    .expect_err("bad target should fail");
    assert!(format!("{err}").contains("unknown PS-3300 pin target bad-target"));
}

fn module_kind(patch: &InstrumentPatch, instance: &str, kind: &str) -> bool {
    patch.modules.iter().any(|module| {
        module.id.as_qualified_str() == format!("audio-synth/ps3300/{instance}")
            && module.kind.as_qualified_str() == format!("audio-synth/module/{kind}")
    })
}

fn section_count(patch: &InstrumentPatch) -> usize {
    patch
        .modules
        .iter()
        .filter(|module| {
            module.kind.as_qualified_str() == "audio-synth/module/ps3-section-generator"
        })
        .count()
}

fn raw_field<'a>(raw_view: &'a PatchRawView, name: &str) -> Option<&'a Expr> {
    raw_view.fields.iter().find_map(|(key, value)| {
        (key.as_qualified_str() == format!("audio-synth/ps3300/{name}")).then_some(value)
    })
}

fn codec_roundtrip(codec: &str, expr: &Expr) -> Expr {
    let mut cx = codec_cx();
    let symbol = Symbol::qualified("codec", codec);
    let output =
        encode_with_codec(&mut cx, &symbol, expr, EncodeOptions::default()).expect("encode patch");
    let input = match output {
        Output::Text(text) => Input::Text(text),
        Output::Bytes(bytes) => Input::Bytes(bytes),
    };
    decode_with_codec(&mut cx, &symbol, input, ReadPolicy::default()).expect("decode patch")
}

fn codec_cx() -> Cx {
    let mut cx = topology_cx();
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&lisp).unwrap();
    let json = JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json).unwrap();
    cx
}

fn topology_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}
