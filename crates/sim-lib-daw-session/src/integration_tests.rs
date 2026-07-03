use sim_kernel::Symbol;
use sim_lib_audio_graph_core::BridgeLatency;
use sim_lib_music_core::{
    Arranger, ArrangerPlacement, Articulation, ChainDevice, ChainPlacement, Channel, ControlEvent,
    LaneId, LaneTarget, Music, Note, ParamSnapshot, ParamValue, PlayContext, PlayEvent,
    PlayableRef, PlayerChain, PlayerMode, PlayerTargetDescriptor, TickTime, TimeRange,
};
use sim_lib_stream_core::{StreamMedia, TransportProfile};
use sim_lib_topology::PlacementNodeProfile;

use crate::{
    DawClip, DawPluginEventExport, DawSessionRouteKind, DawTrack, DawTrackKind, DawTransport,
    instrument_session_fixture, integrate_session_performance,
};

#[test]
fn performance_integration_targets_streams_plugins_and_placement() {
    let mut session = instrument_session_fixture()
        .with_transport(DawTransport::new(true, 128, 120.0).expect("live transport"));
    let arranger = Arranger::new(
        vec![
            targeted_arranger_note("arranged-dx7", 67, "dx7-lead"),
            targeted_arranger_note("arranged-system700", 69, "system700-panel"),
            targeted_arranger_note("arranged-system55", 71, "system55-cabinet"),
            targeted_arranger_note("arranged-ps3300", 72, "ps3300-panel"),
        ],
        vec![LaneId::new("notes")],
    )
    .unwrap();
    let arranger_track = DawTrack::new(
        "arranger-integration",
        "Arranger Integration",
        DawTrackKind::Arranger,
        2,
    )
    .unwrap()
    .with_clip(DawClip::arranger("arranger-clip", 0, 480, arranger).unwrap());
    session.add_track(arranger_track).unwrap();

    let mut player = ChainDevice::new(
        "dual-arp",
        Symbol::qualified("music/player", "dual-arpeggio"),
        PlayerMode::Through,
        0,
    )
    .with_generated(vec![PlayEvent::Control(ControlEvent {
        lane_id: LaneId::new("operator-level"),
        time: TickTime::new(24, 480).unwrap(),
        control: Symbol::qualified("daw-route-target", "operator-level"),
        value: 99,
    })])
    .with_placement(ChainPlacement::new(
        "audio-worklet",
        PlacementNodeProfile::sample_exact(Some(48_000), true)
            .with_latency(BridgeLatency::frames(64)),
    ));
    player.params = ParamSnapshot::new(vec![
        ("rate".to_owned(), ParamValue::I64(8)),
        (
            "scale".to_owned(),
            ParamValue::Symbol(Symbol::qualified("music/scale", "minor")),
        ),
    ]);
    let chain = PlayerChain::new(
        Symbol::qualified("music/source", "keyboard-take"),
        note_music(60),
        vec![player],
        PlayerTargetDescriptor::instrument("dx7-lead"),
    );
    let cx = PlayContext::new(TimeRange::from_ticks(0, 960, 480).unwrap());

    let integrated = integrate_session_performance(&session, &chain, &cx).unwrap();

    assert_binding(
        &integrated,
        "dx7-lead",
        "dx7",
        "dx7-voice",
        DawSessionRouteKind::Midi,
    );
    assert_binding(
        &integrated,
        "dx7-lead",
        "dx7",
        "dx7-voice",
        DawSessionRouteKind::ParameterAutomation,
    );
    assert_binding(
        &integrated,
        "system700-panel",
        "system700",
        "system700",
        DawSessionRouteKind::PatchEdit,
    );
    assert_binding(
        &integrated,
        "system55-cabinet",
        "system55",
        "system55",
        DawSessionRouteKind::Trace,
    );
    assert_binding(
        &integrated,
        "ps3300-panel",
        "ps3300",
        "ps3300",
        DawSessionRouteKind::Preview,
    );
    assert_eq!(integrated.placement_plan().devices.len(), 1);
    assert_eq!(
        integrated.live_schedule().site_symbols(),
        &[Symbol::new("audio-worklet")]
    );
    assert!(integrated.live_schedule().transport().playing);
    assert_eq!(integrated.live_schedule().transport().sample_pos, 128);
    assert!(integrated.live_schedule().bounded_latency_frames() >= 128);
    assert!(integrated.events().len() >= 3);
    assert!(
        integrated
            .stream_envelopes()
            .iter()
            .all(|envelope| envelope.media() == StreamMedia::Data)
    );
    assert!(integrated.stream_envelopes().iter().all(
        |envelope| envelope.profile().name() == TransportProfile::remote_stream_fabric().name()
    ));
    assert!(
        integrated
            .midi_envelopes()
            .iter()
            .any(|envelope| envelope.media() == StreamMedia::Midi
                && envelope.profile().name() == TransportProfile::lan_midi_control().name())
    );
    assert!(
        integrated
            .plugin_events()
            .iter()
            .any(|event| { matches!(event, DawPluginEventExport::NoteOn { key: 60, .. }) })
    );
    assert!(
        integrated
            .plugin_events()
            .iter()
            .any(|event| { matches!(event, DawPluginEventExport::NoteOff { key: 67, .. }) })
    );
    assert!(integrated.plugin_events().iter().any(|event| {
        matches!(
            event,
            DawPluginEventExport::ParamSet { target, value, .. }
                if target.as_qualified_str() == "daw-route-target/operator-level"
                    && *value == 99.0
        )
    }));
    assert!(integrated.automation().iter().any(|automation| {
        automation.device_id() == "dual-arp"
            && automation.enabled()
            && !automation.bypass()
            && automation
                .params()
                .iter()
                .any(|(name, value)| name == "rate" && value == "8")
    }));
    assert!(integrated.frozen_output_hash().starts_with("fnv1a64:"));
    assert!(integrated.trace_count() > 0);
}

fn targeted_arranger_note(id: &'static str, key: u8, target: &'static str) -> ArrangerPlacement {
    ArrangerPlacement::new(
        id,
        PlayableRef::inline(note_music(key)),
        sim_lib_music_core::Time::new(1, 4),
    )
    .unwrap()
    .with_target(LaneTarget::Instrument(Symbol::qualified(
        "music/target",
        target,
    )))
}

fn assert_binding(
    integrated: &crate::DawIntegratedPerformance,
    instrument_id: &'static str,
    kind: &'static str,
    graph_node_id: &'static str,
    route_kind: DawSessionRouteKind,
) {
    assert!(integrated.instrument_bindings().iter().any(|binding| {
        binding.instrument_id().name.as_ref() == instrument_id
            && binding.kind() == kind
            && binding.graph_node_id() == graph_node_id
            && binding.route_kinds().contains(&route_kind)
    }));
}

fn note_music(key: u8) -> Music {
    Music::Note(
        Note::new(
            sim_lib_music_core::Time::new(1, 4),
            sim_lib_music_core::Pitch::from_midi(key),
            100,
            Channel::new(0).unwrap(),
            Articulation::Normal,
        )
        .unwrap(),
    )
}
