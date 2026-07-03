use std::sync::Arc;

use sim_codec::{Input, Output, decode_with_codec, encode_with_codec};
use sim_codec_json::JsonCodecLib;
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{
    Cx, DefaultFactory, EagerPolicy, EncodeOptions, Export, Expr, Lib, ReadPolicy, Symbol,
};
use sim_lib_music_core::{LaneId, LaneKind, TracePolicy};
use sim_lib_music_transform::{
    CustomFilter, DeterminismPolicy, FilterBody, FilterCapability, FilterCapabilitySet, FilterOp,
    FilterPredicate, FilterRule, FilterShape,
};

use crate::{
    MusicShapesLib, custom_filter_from_expr, custom_filter_to_expr, decode_custom_filter,
    encode_custom_filter,
};

#[test]
fn custom_filter_expression_round_trips() {
    let filter = filter_fixture();
    let expr = custom_filter_to_expr(&filter);

    assert_eq!(
        custom_filter_from_expr(&expr).expect("filter from expr"),
        filter
    );
}

#[test]
fn custom_filter_text_round_trips() {
    let filter = filter_fixture();
    let encoded = encode_custom_filter(&filter);

    assert_eq!(
        decode_custom_filter(&encoded).expect("filter from text"),
        filter
    );
}

#[test]
fn custom_filter_round_trips_through_lisp_and_json_codecs() {
    let filter = filter_fixture();
    let expr = custom_filter_to_expr(&filter);

    for codec in ["lisp", "json"] {
        let decoded = codec_roundtrip(codec, &expr);
        assert_eq!(
            custom_filter_from_expr(&decoded).expect("decoded filter"),
            filter,
            "{codec}"
        );
    }
}

#[test]
fn custom_filter_decode_fails_closed_for_invalid_capability() {
    let encoded = encode_custom_filter(&filter_fixture()).replace("rewrite", "filesystem");

    assert!(decode_custom_filter(&encoded).is_err());
}

#[test]
fn music_shapes_lib_exports_custom_filter_shape() {
    let manifest = MusicShapesLib.manifest();

    assert!(manifest.exports.iter().any(|export| {
        matches!(
            export,
            Export::Shape { symbol, .. } if symbol == &Symbol::qualified("music", "CustomFilter")
        )
    }));
}

fn filter_fixture() -> CustomFilter {
    CustomFilter::new(
        "agent-safe-filter",
        FilterShape::notes(),
        FilterShape::new([LaneKind::Note, LaneKind::Control]).expect("output shape"),
        FilterCapabilitySet::rule_ops([
            FilterCapability::Rewrite,
            FilterCapability::Annotate,
            FilterCapability::Sidechain,
        ]),
        DeterminismPolicy::RequiresSeed,
        TracePolicy::Full,
        FilterBody::Rule(vec![
            FilterRule::new(
                FilterPredicate::Lane(LaneId::new("lead")),
                FilterOp::Rewrite {
                    lane: Some(LaneId::new("quantized-lead")),
                    pitch_delta: 0,
                    velocity_delta: -8,
                },
            ),
            FilterRule::new(
                FilterPredicate::Kind(LaneKind::Note),
                FilterOp::Annotate {
                    message: "shape checked".to_owned(),
                },
            ),
            FilterRule::new(
                FilterPredicate::Any,
                FilterOp::Sidechain {
                    lane: LaneId::new("duck"),
                    control: "level".to_owned(),
                },
            ),
        ]),
    )
    .expect("filter")
}

fn codec_roundtrip(codec: &str, expr: &Expr) -> Expr {
    let mut cx = codec_cx();
    let symbol = Symbol::qualified("codec", codec);
    let output =
        encode_with_codec(&mut cx, &symbol, expr, EncodeOptions::default()).expect("encode");
    let input = match output {
        Output::Text(text) => Input::Text(text),
        Output::Bytes(bytes) => Input::Bytes(bytes),
    };
    decode_with_codec(&mut cx, &symbol, input, ReadPolicy::default()).expect("decode")
}

fn codec_cx() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).expect("lisp");
    cx.load_lib(&lisp).expect("load lisp");
    let json = JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json).expect("load json");
    cx
}
