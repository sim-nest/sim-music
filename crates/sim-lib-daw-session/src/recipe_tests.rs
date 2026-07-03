use sim_kernel::Symbol;
use sim_lib_audio_graph_core::BridgeLatency;
use sim_lib_music_core::{
    Articulation, ChainDevice, ChainPlacement, Channel, ControlEvent, LaneId, Music, Note,
    ParamSnapshot, ParamValue, PlayContext, PlayEvent, PlayerChain, PlayerMode,
    PlayerTargetDescriptor, TickTime, TimeRange,
};
use sim_lib_stream_core::StreamMedia;
use sim_lib_topology::PlacementNodeProfile;

use crate::{
    DawPluginEventExport, DawTransport, instrument_session_fixture, integrate_session_performance,
};

fn note_music(key: u8) -> Music {
    Music::Note(
        Note::new(
            sim_lib_music_core::Time::new(1, 4),
            sim_lib_music_core::Pitch::from_midi(key),
            100,
            Channel::new(0).expect("channel"),
            Articulation::Normal,
        )
        .expect("note"),
    )
}

#[test]
fn performance_recipe_targets_sup_instrument_and_freezes_stably() {
    let session = instrument_session_fixture()
        .with_transport(DawTransport::new(true, 256, 120.0).expect("transport"));
    let mut device = ChainDevice::new(
        "bassline-chord",
        Symbol::qualified("music/player", "bassline-generator"),
        PlayerMode::Through,
        0,
    )
    .with_generated(vec![PlayEvent::Control(ControlEvent {
        lane_id: LaneId::new("amp-gain"),
        time: TickTime::new(48, 480).expect("tick"),
        control: Symbol::qualified("daw-route-target", "operator-level"),
        value: 87,
    })])
    .with_placement(ChainPlacement::new(
        "audio-worklet",
        PlacementNodeProfile::sample_exact(Some(48_000), true)
            .with_latency(BridgeLatency::frames(64)),
    ));
    device.params = ParamSnapshot::new(vec![
        ("seed".to_owned(), ParamValue::I64(3824)),
        ("chord-follow".to_owned(), ParamValue::Bool(true)),
        (
            "target".to_owned(),
            ParamValue::Symbol(Symbol::qualified("audio-synth/instrument", "dx7")),
        ),
    ]);
    let chain = PlayerChain::new(
        Symbol::qualified("music/source", "bassline-recipe"),
        note_music(60),
        vec![device],
        PlayerTargetDescriptor::instrument("dx7-lead"),
    );
    let cx = PlayContext::new(TimeRange::from_ticks(0, 960, 480).expect("range"));

    let first = integrate_session_performance(&session, &chain, &cx).expect("first");
    let second = integrate_session_performance(&session, &chain, &cx).expect("second");

    assert_eq!(first.frozen_output_hash(), second.frozen_output_hash());
    assert!(first.frozen_output_hash().starts_with("fnv1a64:"));
    assert_eq!(first.placement_plan().devices.len(), 1);
    assert_eq!(
        first.live_schedule().site_symbols(),
        &[Symbol::new("audio-worklet")]
    );
    assert!(first.instrument_bindings().iter().any(|binding| {
        binding.instrument_id().name.as_ref() == "dx7-lead" && binding.kind() == "dx7"
    }));
    assert!(
        first
            .stream_envelopes()
            .iter()
            .all(|envelope| envelope.media() == StreamMedia::Data)
    );
    assert!(
        first
            .midi_envelopes()
            .iter()
            .any(|envelope| envelope.media() == StreamMedia::Midi)
    );
    assert!(first.plugin_events().iter().any(|event| {
        matches!(
            event,
            DawPluginEventExport::ParamSet { target, value, .. }
                if target.as_qualified_str() == "daw-route-target/operator-level"
                    && *value == 87.0
        )
    }));
    assert!(first.automation().iter().any(|automation| {
        automation.device_id() == "bassline-chord"
            && automation
                .params()
                .iter()
                .any(|(name, value)| name == "seed" && value == "3824")
    }));
}

#[test]
fn daw_recipe_sources_are_registered_for_generated_docs() {
    for source in [
        include_str!(
            "../recipes/02-performance-integration/sup-instrument-performance/recipe.toml"
        ),
        include_str!(
            "../recipes/02-performance-integration/golden-cassette-integration/recipe.toml"
        ),
    ] {
        assert!(source.contains("golden"));
        assert!(source.contains("codec = \"lisp\""));
    }
}
