use std::sync::Arc;

use sim_codec::{Input, Output, decode_with_codec, encode_with_codec};
use sim_codec_json::JsonCodecLib;
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, EncodeOptions, Expr, ReadPolicy, Symbol};
use sim_lib_topology::validate::validate_graph;

use crate::{
    InstrumentPatch, PatchCord, PatchEndpoint, PatchJack, PatchModule, PatchRawView, SynthPreset,
    subtractive_synth_algorithm_patch,
};

#[test]
fn instrument_patch_round_trips_as_expr() {
    let patch = modular_patch();
    let expr = patch.to_expr();

    assert_eq!(InstrumentPatch::from_expr(&expr).expect("patch"), patch);
}

#[test]
fn instrument_patch_round_trips_through_lisp_and_json_codecs() {
    let patch = modular_patch();
    let expr = patch.to_expr();

    for codec in ["lisp", "json"] {
        let decoded = codec_roundtrip(codec, &expr);
        assert_eq!(
            InstrumentPatch::from_expr(&decoded).expect("decoded patch"),
            patch,
            "{codec}"
        );
    }
}

#[test]
fn subtractive_synth_patch_exposes_fixed_algorithm_as_topology_data() {
    let patch = subtractive_synth_algorithm_patch(&SynthPreset::default());
    let graph = patch.topology_graph();

    validate_graph(&mut topology_cx(), &graph).expect("subtractive synth topology");
    assert_eq!(graph.nodes.len(), 4);
    assert_eq!(graph.edges.len(), 3);
    assert!(
        graph
            .metadata
            .iter()
            .any(|(key, _)| key == &Symbol::qualified("topology", "adapter"))
    );
    let amp = graph
        .nodes
        .iter()
        .find(|node| node.id.as_symbol() == &Symbol::new("amp"))
        .expect("amp node");
    assert!(
        amp.options
            .iter()
            .any(|(key, _)| key.name.as_ref() == "normalled-defaults")
    );
}

fn modular_patch() -> InstrumentPatch {
    InstrumentPatch::new(Symbol::qualified("audio-synth", "modular-test"))
        .with_setting(Symbol::new("voice-mode"), Expr::String("mono".to_owned()))
        .with_raw_view(
            PatchRawView::new(Symbol::qualified("audio-synth/raw", "synthetic"))
                .with_field(Symbol::new("slot"), Expr::String("A01".to_owned())),
        )
        .with_module(
            PatchModule::new(Symbol::new("in"), Symbol::new("in"))
                .with_output(PatchJack::control("out", true)),
        )
        .with_module(
            PatchModule::new(
                Symbol::new("osc"),
                Symbol::qualified("audio-synth", "oscillator"),
            )
            .with_input(PatchJack::control("pitch", true))
            .with_input(PatchJack::gate("gate", true).with_normalled_default(Expr::Bool(true)))
            .with_output(PatchJack::audio("audio", true))
            .with_setting(
                Symbol::new("waveform"),
                Expr::Symbol(Symbol::qualified("audio-synth", "polyblep-saw")),
            ),
        )
        .with_module(
            PatchModule::new(Symbol::new("out"), Symbol::new("out"))
                .with_input(PatchJack::audio("in", true)),
        )
        .with_cord(PatchCord::new(
            PatchEndpoint::new("in", "out"),
            PatchEndpoint::new("osc", "pitch"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("osc", "audio"),
            PatchEndpoint::new("out", "in"),
        ))
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
