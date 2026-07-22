use std::path::Path;

// conformance: synth performance workbench renders checked DAW descriptors.

use sim_lib_audio_graph_live::LiveStreamLane;

use crate::daw::{
    ALL_LOCAL_PLACEMENT_RECIPE_PATH, BROWSER_WASM_LOCAL_RECIPE_PATH,
    DAW_INSTRUMENT_RENDER_SMOKE_COMMAND, DX7_LOCAL_STREAM_RECIPE_PATH, DawInstrumentGraphKind,
    GENERIC_INSTRUMENT_STREAM_RECIPE_PATH, MODULAR_LOCAL_STREAM_RECIPE_PATH,
    VOICE_LOCAL_LAN_PREVIEW_RECIPE_PATH, VOICE_LOCAL_SERVER_FX_RECIPE_PATH,
    daw_instrument_audio_graph, daw_instrument_graph_node_descriptors,
    daw_instrument_graph_node_ids, daw_instrument_render_smoke_command,
    daw_instrument_session_fixture_names, instrument_placement_recipe_specs,
    instrument_recipe_paths, instrument_stream_recipe_specs,
};

#[test]
fn daw_instrument_graph_nodes_render_smoke() {
    assert_eq!(
        daw_instrument_render_smoke_command(),
        DAW_INSTRUMENT_RENDER_SMOKE_COMMAND
    );
    assert_eq!(
        daw_instrument_session_fixture_names(),
        [
            "dx7-session-graph",
            "system700-session-graph",
            "system55-session-graph",
            "ps3300-session-graph",
        ]
    );
    assert_eq!(
        daw_instrument_graph_node_ids(),
        ["dx7-voice", "system700", "system55", "ps3300"]
    );

    for descriptor in daw_instrument_graph_node_descriptors() {
        let mut graph = daw_instrument_audio_graph(descriptor.kind()).expect("graph wrapper");
        let patch = graph.to_patch();
        assert_eq!(patch.nodes.len(), 1);
        assert_eq!(patch.nodes[0].id, descriptor.node_id());
        assert!(!descriptor.component_id().as_qualified_str().is_empty());
        assert!(
            descriptor
                .render_smoke_command()
                .contains("daw_instrument_graph_nodes_render_smoke")
        );
        assert!(
            daw_instrument_session_fixture_names()
                .iter()
                .any(|fixture| *fixture == descriptor.session_fixture())
        );

        graph.prepare(48_000, 8).expect("prepare graph");
        let output = graph.process_offline(&[], 4).expect("render graph");
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].len(), 4);
        assert!(
            output[0].iter().all(|sample| sample.is_finite()),
            "non-finite output from {}",
            descriptor.kind().as_str()
        );
    }

    assert_eq!(
        DawInstrumentGraphKind::parse_name("system55").unwrap(),
        DawInstrumentGraphKind::System55
    );
    assert!(DawInstrumentGraphKind::parse_name("unknown").is_err());
}

#[test]
fn instrument_stream_and_placement_recipes_record_stable_artifacts() {
    assert_eq!(
        instrument_recipe_paths(),
        vec![
            GENERIC_INSTRUMENT_STREAM_RECIPE_PATH,
            DX7_LOCAL_STREAM_RECIPE_PATH,
            MODULAR_LOCAL_STREAM_RECIPE_PATH,
            ALL_LOCAL_PLACEMENT_RECIPE_PATH,
            VOICE_LOCAL_SERVER_FX_RECIPE_PATH,
            VOICE_LOCAL_LAN_PREVIEW_RECIPE_PATH,
            BROWSER_WASM_LOCAL_RECIPE_PATH,
        ]
    );
    for path in instrument_recipe_paths() {
        assert!(recipe_exists(path), "missing recipe path {path}");
    }

    let live_stream_ids = LiveStreamLane::all()
        .iter()
        .map(|lane| lane.stream_id().as_qualified_str())
        .collect::<Vec<_>>();
    let stream_specs = instrument_stream_recipe_specs();
    assert_eq!(
        stream_specs
            .iter()
            .map(|spec| spec.id())
            .collect::<Vec<_>>(),
        [
            "generic-instrument-stream",
            "dx7-local-stream",
            "modular-local-stream"
        ]
    );
    assert!(
        stream_specs[0]
            .stream_ids()
            .iter()
            .any(|stream| live_stream_ids.iter().any(|live| live.as_str() == *stream))
    );
    assert!(stream_specs[0].stream_ids().contains(&"stream/live/patch"));
    assert!(
        stream_specs[0]
            .stream_ids()
            .contains(&"stream/live/preview")
    );
    assert!(
        stream_specs[1].refuses_remote_hard_realtime()
            && stream_specs[2].refuses_remote_hard_realtime()
    );
    for spec in stream_specs {
        assert!(spec.latency_budget().contains("audio"));
        assert!(
            spec.artifact_names()
                .iter()
                .any(|artifact| artifact.ends_with(".sha256")),
            "{} must name a hash artifact",
            spec.id()
        );
    }

    let placement_specs = instrument_placement_recipe_specs();
    assert_eq!(
        placement_specs
            .iter()
            .map(|spec| spec.id())
            .collect::<Vec<_>>(),
        [
            "all-local",
            "voice-local-server-fx",
            "voice-local-lan-preview",
            "browser-wasm-local"
        ]
    );
    assert!(
        placement_specs
            .iter()
            .any(|spec| spec.site_map().contains("stream/site/host-callback"))
    );
    assert!(placement_specs.iter().any(|spec| {
        spec.site_map()
            .contains("stream/profile/lan-buffered-audio-preview")
    }));
    assert!(
        placement_specs
            .iter()
            .any(|spec| spec.site_map().contains("stream/site/browser-wasm"))
    );
    for spec in placement_specs {
        assert!(spec.stream_ids().contains(&"stream/live/audio-output"));
        assert!(!spec.latency_budget().is_empty());
        assert!(
            spec.artifact_names()
                .iter()
                .any(|artifact| artifact.ends_with(".sha256")),
            "{} must name a hash artifact",
            spec.id()
        );
    }
}

fn recipe_exists(path: &str) -> bool {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/");
    if repo_root.join(path).exists() {
        return true;
    }
    std::env::current_dir().ok().is_some_and(|dir| {
        dir.ancestors().any(|ancestor| {
            ancestor.join(path).exists()
                || ancestor.join("../sim-music").join(path).exists()
                || ancestor
                    .parent()
                    .is_some_and(|parent| parent.join("sim-music").join(path).exists())
        })
    })
}
