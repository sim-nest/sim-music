use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use sim_codec::encode_with_codec;
use sim_codec_algol::AlgolCodecLib;
use sim_codec_binary::BinaryCodecLib;
use sim_codec_binary_base64::BinaryBase64CodecLib;
use sim_codec_json::JsonCodecLib;
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{
    DefaultFactory, Dir, EagerPolicy, EncodeOptions, Expr, ObjectEncoding, Symbol,
    read_construct_capability,
};

use crate::{
    FsDir, FsDirDescriptor, fs_dir_class_symbol, install_fs_dir_lib, table_fs_capability,
    table_fs_mkdir_capability, table_fs_read_capability, table_fs_rmdir_capability,
    table_fs_write_capability,
};

fn test_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "sim-table-fs-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn cx() -> sim_kernel::Cx {
    let mut cx = sim_kernel::Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    let lisp_id = cx.registry_mut().fresh_codec_id();
    cx.load_lib(&LispCodecLib::new(lisp_id).unwrap()).unwrap();
    let json_id = cx.registry_mut().fresh_codec_id();
    cx.load_lib(&JsonCodecLib::new(json_id)).unwrap();
    let binary_id = cx.registry_mut().fresh_codec_id();
    cx.load_lib(&BinaryCodecLib::new(binary_id)).unwrap();
    let binary_base64_id = cx.registry_mut().fresh_codec_id();
    cx.load_lib(&BinaryBase64CodecLib::new(binary_base64_id))
        .unwrap();
    let algol_id = cx.registry_mut().fresh_codec_id();
    cx.load_lib(&AlgolCodecLib::new(algol_id)).unwrap();
    cx
}

fn grant(cx: &mut sim_kernel::Cx, capabilities: &[sim_kernel::CapabilityName]) {
    for capability in capabilities {
        cx.grant(capability.clone());
    }
}

fn write_value_with_codec(cx: &mut sim_kernel::Cx, path: &PathBuf, codec: Symbol, value: &str) {
    let expr = Expr::String(value.to_owned());
    let output = encode_with_codec(cx, &codec, &expr, EncodeOptions::default()).unwrap();
    let bytes = match output {
        sim_codec::Output::Text(text) => text.into_bytes(),
        sim_codec::Output::Bytes(bytes) => bytes,
    };
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn fs_dir_set_get_roundtrip_and_extension_selection() {
    let mut cx = cx();
    grant(
        &mut cx,
        &[
            table_fs_capability(),
            table_fs_read_capability(),
            table_fs_write_capability(),
        ],
    );

    let root = test_root("roundtrip");
    let dir = install_fs_dir_lib(&mut cx, root.to_str().unwrap()).unwrap();
    let table = dir.object().as_table_impl().unwrap();
    let hello = cx.factory().string("hello".to_owned()).unwrap();

    table.set(&mut cx, Symbol::new("alpha"), hello).unwrap();
    assert!(root.join("alpha.siml").is_file());
    let alpha = table.get(&mut cx, Symbol::new("alpha")).unwrap();
    assert_eq!(
        alpha.object().as_expr(&mut cx).unwrap(),
        Expr::String("hello".to_owned())
    );

    write_value_with_codec(
        &mut cx,
        &root.join("json-value.simj"),
        Symbol::qualified("codec", "json"),
        "json",
    );
    write_value_with_codec(
        &mut cx,
        &root.join("binary-value.simb"),
        Symbol::qualified("codec", "binary"),
        "binary",
    );
    write_value_with_codec(
        &mut cx,
        &root.join("binary-base64-value.simb64"),
        Symbol::qualified("codec", "binary-base64"),
        "binary-base64",
    );
    write_value_with_codec(
        &mut cx,
        &root.join("algol-value.sima"),
        Symbol::qualified("codec", "algol"),
        "algol",
    );

    for (key, expected) in [
        ("json-value", "json"),
        ("binary-value", "binary"),
        ("binary-base64-value", "binary-base64"),
        ("algol-value", "algol"),
    ] {
        let value = table.get(&mut cx, Symbol::new(key)).unwrap();
        assert_eq!(
            value.object().as_expr(&mut cx).unwrap(),
            Expr::String(expected.to_owned())
        );
    }
}

#[test]
fn fs_dir_mkdir_opendir_rmdir_and_path_guards() {
    let mut cx = cx();
    grant(
        &mut cx,
        &[
            table_fs_capability(),
            table_fs_read_capability(),
            table_fs_mkdir_capability(),
            table_fs_rmdir_capability(),
        ],
    );

    let root = test_root("dirs");
    let dir = FsDir::open(root.clone()).unwrap();
    let sub = dir.mkdir(&mut cx, Symbol::new("sub")).unwrap();
    assert!(sub.object().as_dir().is_some());
    assert!(dir.is_dir(&mut cx, Symbol::new("sub")).unwrap());
    assert!(dir.opendir(&mut cx, Symbol::new("sub")).unwrap().is_some());
    dir.rmdir(&mut cx, Symbol::new("sub")).unwrap();
    assert!(!root.join("sub").exists());

    for illegal in ["/tmp", "..", ".", "a/b", "a\\b"] {
        let err = dir.mkdir(&mut cx, Symbol::new(illegal)).unwrap_err();
        assert!(err.to_string().contains("illegal name"));
    }
}

#[test]
fn fs_dir_operations_are_capability_gated() {
    let mut cx = cx();
    grant(&mut cx, &[table_fs_capability()]);

    let root = test_root("caps");
    let dir = install_fs_dir_lib(&mut cx, root.to_str().unwrap()).unwrap();
    let table = dir.object().as_table_impl().unwrap();
    let fs_dir = dir.object().as_dir().unwrap();
    let value = cx.factory().string("value".to_owned()).unwrap();

    assert!(matches!(
        table.get(&mut cx, Symbol::new("x")),
        Err(sim_kernel::Error::CapabilityDenied { .. })
    ));
    assert!(matches!(
        table.set(&mut cx, Symbol::new("x"), value),
        Err(sim_kernel::Error::CapabilityDenied { .. })
    ));
    assert!(matches!(
        fs_dir.mkdir(&mut cx, Symbol::new("sub")),
        Err(sim_kernel::Error::CapabilityDenied { .. })
    ));
    assert!(matches!(
        fs_dir.rmdir(&mut cx, Symbol::new("sub")),
        Err(sim_kernel::Error::CapabilityDenied { .. })
    ));
}

#[test]
fn fs_dir_rejects_traversal_inputs_for_get_set_and_mkdir() {
    let mut cx = cx();
    grant(
        &mut cx,
        &[
            table_fs_capability(),
            table_fs_read_capability(),
            table_fs_write_capability(),
            table_fs_mkdir_capability(),
        ],
    );

    let root = test_root("traversal");
    let dir = install_fs_dir_lib(&mut cx, root.to_str().unwrap()).unwrap();
    let table = dir.object().as_table_impl().unwrap();
    let fs_dir = dir.object().as_dir().unwrap();
    let value = cx.factory().string("value".to_owned()).unwrap();

    for illegal in ["..", "/etc/passwd", "a/b", "a\\b"] {
        let key = Symbol::new(illegal);

        let err = table.get(&mut cx, key.clone()).unwrap_err();
        assert!(err.to_string().contains("illegal name") || err.to_string().contains("escapes"));

        let err = table.set(&mut cx, key.clone(), value.clone()).unwrap_err();
        assert!(err.to_string().contains("illegal name") || err.to_string().contains("escapes"));

        let err = fs_dir.mkdir(&mut cx, key).unwrap_err();
        assert!(err.to_string().contains("illegal name") || err.to_string().contains("escapes"));
    }

    assert!(!root.join("a").exists());
    assert!(!root.join("etc").exists());
}

#[test]
fn fs_dir_rejects_illegal_segments_via_shared_predicate() {
    let mut cx = cx();
    grant(
        &mut cx,
        &[
            table_fs_capability(),
            table_fs_read_capability(),
            table_fs_write_capability(),
            table_fs_mkdir_capability(),
        ],
    );

    let root = test_root("illegal-segments");
    let dir = install_fs_dir_lib(&mut cx, root.to_str().unwrap()).unwrap();
    let table = dir.object().as_table_impl().unwrap();
    let fs_dir = dir.object().as_dir().unwrap();

    // Names that `is_legal_table_segment` rejects, plus the table-fs-only
    // absolute-path guard, must all fail closed with the table-fs message.
    for illegal in ["", ".", "..", "a/b", "a\\b", "/abs"] {
        assert!(!sim_table_core::is_legal_table_segment(illegal) || illegal == "/abs");
        let key = Symbol::new(illegal);
        let err = table.get(&mut cx, key.clone()).unwrap_err();
        assert!(err.to_string().contains("illegal name") || err.to_string().contains("escapes"));
        let err = fs_dir.mkdir(&mut cx, key).unwrap_err();
        assert!(err.to_string().contains("illegal name") || err.to_string().contains("escapes"));
    }
}

#[test]
fn fs_dir_citizen_round_trips_as_descriptor_only() {
    let mut cx = cx();
    cx.load_lib(&sim_citizen::CitizenLib::all()).unwrap();
    cx.grant(read_construct_capability());
    let root = test_root("citizen");
    let dir = FsDir::open(root).unwrap();
    let original = cx.factory().opaque(Arc::new(dir)).unwrap();

    sim_citizen::check_value_fixture(&mut cx, original.clone()).unwrap();

    let ObjectEncoding::Constructor { args, .. } = original
        .object()
        .as_object_encoder()
        .unwrap()
        .object_encoding(&mut cx)
        .unwrap()
    else {
        panic!("expected constructor encoding");
    };
    let args = args
        .iter()
        .map(|arg| sim_citizen::value_from_expr(&mut cx, arg))
        .collect::<sim_kernel::Result<Vec<_>>>()
        .unwrap();
    let decoded = cx.read_construct(&fs_dir_class_symbol(), args).unwrap();

    assert!(
        decoded
            .object()
            .as_any()
            .downcast_ref::<FsDirDescriptor>()
            .is_some()
    );
    assert!(decoded.object().as_table_impl().is_none());
    assert!(decoded.object().as_dir().is_none());
}

#[cfg(feature = "music")]
#[test]
fn fs_dir_reads_music_artifacts_without_eval_capabilities() {
    let mut cx = cx();
    grant(
        &mut cx,
        &[table_fs_capability(), table_fs_read_capability()],
    );
    let root = test_root("music-artifact");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("score.music"),
        "#(Score tempo=120 time_sig=4/4 key=\"C major\" body=#(Melody items=[#(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal)]))",
    )
    .unwrap();

    let dir = install_fs_dir_lib(&mut cx, root.to_str().unwrap()).unwrap();
    let value = dir
        .object()
        .as_table_impl()
        .unwrap()
        .get(&mut cx, Symbol::new("score"))
        .unwrap();
    let expr = value.object().as_expr(&mut cx).unwrap();
    assert_eq!(
        expr,
        Expr::Extension {
            tag: Symbol::qualified("music", "Score"),
            payload: Box::new(Expr::String(
                "#(Score tempo=120 time_sig=4/4 key=\"C major\" body=#(Melody items=[#(Note dur=1/4 pitch=C4 vel=100 channel=0 articulation=Normal)]))".to_owned()
            )),
        }
    );
}

#[cfg(feature = "sound")]
#[test]
fn fs_dir_reads_and_rewrites_tone_artifacts() {
    let mut cx = cx();
    grant(
        &mut cx,
        &[
            table_fs_capability(),
            table_fs_read_capability(),
            table_fs_write_capability(),
        ],
    );
    let root = test_root("tone-artifact");
    std::fs::create_dir_all(&root).unwrap();
    let canonical = "#(Tone partials=[#(Partial frequency=#(Frequency hz=440) amplitude=#(Amplitude linear=1) phase=#(Phase radians=0))] envelope=#(Envelope attack=0.015 decay=0.06 sustain=0.75 release=0.12 shape=#(EnvelopeShape kind=Linear)) duration=1)";
    std::fs::write(root.join("tone.tone"), canonical).unwrap();

    let dir = install_fs_dir_lib(&mut cx, root.to_str().unwrap()).unwrap();
    let table = dir.object().as_table_impl().unwrap();
    let value = table.get(&mut cx, Symbol::new("tone")).unwrap();
    table.set(&mut cx, Symbol::new("tone"), value).unwrap();

    let rewritten = std::fs::read_to_string(root.join("tone.tone")).unwrap();
    assert_eq!(rewritten, canonical);
}
