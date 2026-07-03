use num_rational::Ratio;
use sim_kernel::Symbol;
use sim_lib_stream_core::RateContract;
use sim_lib_topology::PlacementNodeProfile;

use crate::{
    Articulation, ChainDevice, ChainPlacement, ChainPlacementPlan, Channel, LaneId, Music, Note,
    NoteEvent, ParamSnapshot, ParamValue, PlayContext, PlayEvent, Playable, PlayerChain,
    PlayerMode, PlayerTargetDescriptor, TickTime, Time, TimeRange, TraceAction,
};

fn quarter() -> Time {
    Ratio::new(1, 4)
}

fn note(midi: u8) -> Note {
    Note::new(
        quarter(),
        crate::Pitch::from_midi(midi),
        100,
        Channel::new(0).expect("channel"),
        Articulation::Normal,
    )
    .expect("note")
}

fn tick(ticks: i64) -> TickTime {
    TickTime::new(ticks, 480).expect("tick")
}

fn play_context() -> PlayContext {
    PlayContext::new(TimeRange::from_ticks(0, 960, 480).expect("range"))
}

fn note_event(lane: &str, midi: u8) -> PlayEvent {
    PlayEvent::Note(NoteEvent {
        lane_id: LaneId::new(lane),
        time: tick(0),
        duration: tick(120),
        pitch: crate::Pitch::from_midi(midi),
        velocity: 100,
        channel: Channel::new(0).expect("channel"),
    })
}

fn player_symbol(name: &str) -> Symbol {
    Symbol::qualified("music/player", name)
}

fn source_chain(devices: Vec<ChainDevice>) -> PlayerChain {
    PlayerChain::new(
        Symbol::qualified("music/source", "unit"),
        Music::Note(note(60)),
        devices,
        PlayerTargetDescriptor::instrument("default"),
    )
}

fn note_midis(events: &[PlayEvent]) -> Vec<u8> {
    let mut midis = events
        .iter()
        .filter_map(|event| match event {
            PlayEvent::Note(note) => note.pitch.to_midi(),
            _ => None,
        })
        .collect::<Vec<_>>();
    midis.sort();
    midis
}

#[test]
fn player_modes_and_params_have_stable_wire_forms() {
    assert_eq!(PlayerMode::Through.wire_label(), "through");
    assert_eq!(PlayerMode::Replace.wire_label(), "replace");
    assert_eq!(PlayerMode::Filter.wire_label(), "filter");
    assert_eq!(PlayerMode::Sidechain.wire_label(), "sidechain");
    assert_eq!(PlayerMode::SelfClocked.wire_label(), "self_clocked");

    let params = ParamSnapshot::new(vec![
        ("z".to_owned(), ParamValue::I64(2)),
        ("a".to_owned(), ParamValue::Bool(true)),
    ]);
    assert_eq!(params.entries[0].0, "a");
    assert_eq!(params.entries[1].0, "z");
}

#[test]
fn player_chain_order_and_bypass_are_stable() {
    let chain = source_chain(vec![
        ChainDevice::new("c", player_symbol("c"), PlayerMode::Through, 2)
            .with_generated(vec![note_event("c", 67)]),
        ChainDevice::new("a", player_symbol("a"), PlayerMode::Through, 0)
            .with_generated(vec![note_event("a", 64)])
            .bypassed(),
        ChainDevice::new("b", player_symbol("b"), PlayerMode::Through, 1)
            .with_generated(vec![note_event("b", 65)]),
    ]);

    let stable_ids = chain
        .stable_devices()
        .iter()
        .map(|device| device.id.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(stable_ids, vec!["a", "b", "c"]);

    let render = chain.render_chain(&play_context()).expect("render");

    assert_eq!(note_midis(&render.events), vec![60, 65, 67]);
    assert_eq!(render.traces[0].device_id.as_ref(), "b");
    assert_eq!(render.traces[0].action, TraceAction::Routed);
    assert!(
        render
            .traces
            .iter()
            .all(|trace| trace.device_id.as_ref() != "a")
    );
}

#[test]
fn player_chain_solo_mute_and_enable_control_selection() {
    let mut muted_solo = ChainDevice::new("muted", player_symbol("muted"), PlayerMode::Replace, 0)
        .with_generated(vec![note_event("muted", 61)]);
    muted_solo.solo = true;
    muted_solo.mute = true;

    let mut solo = ChainDevice::new("solo", player_symbol("solo"), PlayerMode::Replace, 1)
        .with_generated(vec![note_event("solo", 62)]);
    solo.solo = true;

    let mut disabled = ChainDevice::new(
        "disabled",
        player_symbol("disabled"),
        PlayerMode::Replace,
        2,
    )
    .with_generated(vec![note_event("disabled", 63)]);
    disabled.enabled = false;

    let chain = source_chain(vec![muted_solo, solo, disabled]);
    let render = chain.render_chain(&play_context()).expect("render");

    assert_eq!(note_midis(&render.events), vec![62]);
    assert!(
        render
            .traces
            .iter()
            .all(|trace| trace.device_id.as_ref() == "solo")
    );
}

#[test]
fn player_chain_records_source_and_direct_output() {
    let chain = source_chain(vec![
        ChainDevice::new("gen", player_symbol("gen"), PlayerMode::Through, 0)
            .with_generated(vec![note_event("gen", 65)]),
    ]);
    let cx = play_context();

    let source = chain.record_source(&cx);
    let direct = chain.record_direct(&cx).expect("direct");

    assert_eq!(source.source_id, chain.source_id);
    assert_eq!(source.chain_hash, chain.chain_hash());
    assert_eq!(source.seed, cx.seed);
    assert_eq!(direct.events.len(), 2);
    assert_eq!(direct.meta.chain_hash, source.chain_hash);
    assert_eq!(direct.meta.context_hash, source.context_hash);
}

#[test]
fn player_chain_freeze_is_deterministic() {
    let chain = source_chain(vec![
        ChainDevice::new("gen", player_symbol("gen"), PlayerMode::Through, 0)
            .with_generated(vec![note_event("gen", 65)]),
    ]);
    let cx = play_context();

    let first = chain.freeze_chain(&cx).expect("first");
    let second = chain.freeze_chain(&cx).expect("second");
    let playable = chain.freeze(&cx).expect("playable freeze");

    assert_eq!(first, second);
    assert_eq!(playable.content_hash, first.meta.output_hash);
}

#[test]
fn player_chain_placement_round_trips() {
    let chain = source_chain(vec![
        ChainDevice::new("local", player_symbol("local"), PlayerMode::Through, 0),
        ChainDevice::new("worker", player_symbol("worker"), PlayerMode::Through, 1).with_placement(
            ChainPlacement::new(
                "worker",
                PlacementNodeProfile::new(RateContract::control(), false),
            ),
        ),
    ]);
    let plan = chain.placement_plan();
    let expr = plan.to_expr();

    assert_eq!(
        ChainPlacementPlan::from_expr(&expr).expect("placement round-trip"),
        plan
    );
}

#[test]
fn player_chain_trace_actions_are_stable() {
    let chain = source_chain(vec![
        ChainDevice::new("route", player_symbol("route"), PlayerMode::Through, 0),
        ChainDevice::new("rewrite", player_symbol("rewrite"), PlayerMode::Through, 1)
            .with_route_lane(LaneId::new("routed")),
        ChainDevice::new("drop", player_symbol("drop"), PlayerMode::Filter, 2)
            .with_filter_lanes(vec![LaneId::new("routed")]),
        ChainDevice::new("clock", player_symbol("clock"), PlayerMode::SelfClocked, 3)
            .with_generated(vec![note_event("clock", 72)]),
    ]);

    let render = chain.render_chain(&play_context()).expect("render");
    let actions = render
        .traces
        .iter()
        .map(|trace| trace.action)
        .collect::<Vec<_>>();

    assert_eq!(
        actions,
        vec![
            TraceAction::Routed,
            TraceAction::Rewritten,
            TraceAction::Dropped,
            TraceAction::Generated,
        ]
    );
    assert_eq!(note_midis(&render.events), vec![72]);
}
