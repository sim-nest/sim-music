use sim_lib_music_core::{
    Channel, ControlEvent, LaneId, LaneKind, NoteEvent, PlayEvent, Tick, TracePolicy,
};
use sim_lib_pitch_core::Pitch;

use crate::{
    CallableFilterDefinition, CallableFilterRef, CallableFilterRegistry, CustomFilter,
    CustomFilterError, DeterminismPolicy, FilterBody, FilterCapability, FilterCapabilitySet,
    FilterContext, FilterOp, FilterPredicate, FilterRule, FilterShape, FilterTraceAction,
    ensure_custom_filter_codec,
};

#[test]
fn custom_filter_rule_ops_transform_events_and_trace() {
    let source = note_event("lead", 14, 60, 90);

    let accepted = run_op(FilterOp::Accept, vec![source.clone()], FilterShape::notes());
    assert_eq!(accepted.events, vec![source.clone()]);
    assert_eq!(accepted.traces[0].action, FilterTraceAction::Accepted);

    let rejected = run_op(FilterOp::Reject, vec![source.clone()], FilterShape::notes());
    assert!(rejected.events.is_empty());
    assert_eq!(rejected.traces[0].action, FilterTraceAction::Rejected);

    let cloned = run_op(
        FilterOp::Clone { copies: 2 },
        vec![source.clone()],
        FilterShape::notes(),
    );
    assert_eq!(cloned.events.len(), 3);

    let rewritten = run_op(
        FilterOp::Rewrite {
            lane: Some(LaneId::new("rewritten")),
            pitch_delta: 2,
            velocity_delta: -20,
        },
        vec![source.clone()],
        FilterShape::notes(),
    );
    let PlayEvent::Note(note) = &rewritten.events[0] else {
        panic!("note");
    };
    assert_eq!(note.lane_id, LaneId::new("rewritten"));
    assert_eq!(note.pitch.to_midi(), Some(62));
    assert_eq!(note.velocity, 70);

    let routed = run_op(
        FilterOp::Route {
            lane: LaneId::new("bus"),
        },
        vec![source.clone()],
        FilterShape::notes(),
    );
    assert_eq!(routed.events[0].lane_id(), &LaneId::new("bus"));

    let annotated = run_op(
        FilterOp::Annotate {
            message: "kept for downstream agent".to_owned(),
        },
        vec![source.clone()],
        FilterShape::notes(),
    );
    assert_eq!(annotated.events, vec![source.clone()]);
    assert_eq!(annotated.traces[0].message, "kept for downstream agent");

    let quantized = run_op(
        FilterOp::Quantize { grid_ticks: 10 },
        vec![source.clone()],
        FilterShape::notes(),
    );
    assert_eq!(quantized.events[0].time().ticks, 10);

    let thinned = run_op(
        FilterOp::Thin { keep_every: 2 },
        vec![
            note_event("lead", 0, 60, 90),
            note_event("lead", 10, 62, 90),
            note_event("lead", 20, 64, 90),
        ],
        FilterShape::notes(),
    );
    assert_eq!(thinned.events.len(), 2);
    assert_eq!(thinned.events[0].time().ticks, 0);
    assert_eq!(thinned.events[1].time().ticks, 20);

    let expanded = run_op(
        FilterOp::Expand {
            copies: 2,
            step_ticks: 5,
        },
        vec![source.clone()],
        FilterShape::notes(),
    );
    assert_eq!(
        expanded
            .events
            .iter()
            .map(|event| event.time().ticks)
            .collect::<Vec<_>>(),
        vec![14, 19, 24]
    );

    let sidechained = run_op(
        FilterOp::Sidechain {
            lane: LaneId::new("duck"),
            control: "level".to_owned(),
        },
        vec![source],
        FilterShape::new([LaneKind::Note, LaneKind::Control]).expect("shape"),
    );
    assert_eq!(sidechained.events.len(), 2);
    assert!(
        sidechained
            .events
            .iter()
            .any(|event| matches!(event, PlayEvent::Control(ControlEvent { value: 90, .. })))
    );
}

#[test]
fn custom_filter_fails_closed_for_capability_shape_and_codec() {
    let filter = filter_for_op(
        FilterOp::Route {
            lane: LaneId::new("bus"),
        },
        FilterShape::notes(),
    );
    let context = FilterContext::new(FilterCapabilitySet::new([FilterCapability::Rule]));
    assert!(matches!(
        filter.evaluate(&context, vec![note_event("lead", 0, 60, 80)]),
        Err(CustomFilterError::MissingCapability(cap)) if cap == "route"
    ));

    let shape_error = filter
        .evaluate(
            &FilterContext::all_capabilities(),
            vec![control_event("ctl", 0, 7)],
        )
        .expect_err("shape mismatch");
    assert!(matches!(
        shape_error,
        CustomFilterError::ShapeMismatch {
            phase: "input",
            kind: "control"
        }
    ));

    assert!(matches!(
        ensure_custom_filter_codec("yaml"),
        Err(CustomFilterError::UnsupportedCodec(codec)) if codec == "yaml"
    ));
}

#[test]
fn callable_filters_are_declared_deterministic_and_eval_gated() {
    let callable_filter = CustomFilter::new(
        "callable-test",
        FilterShape::notes(),
        FilterShape::notes(),
        FilterCapabilitySet::new([FilterCapability::Callable]),
        DeterminismPolicy::Deterministic,
        TracePolicy::Full,
        FilterBody::Callable(CallableFilterRef::new("route-callable")),
    )
    .expect("filter");
    let mut registry = CallableFilterRegistry::default();
    registry.register(CallableFilterDefinition::new(
        "route-callable",
        vec![FilterRule::new(
            FilterPredicate::Any,
            FilterOp::Route {
                lane: LaneId::new("callable-out"),
            },
        )],
    ));

    let run = callable_filter
        .evaluate_with_registry(
            &FilterContext::all_capabilities(),
            vec![note_event("lead", 0, 60, 80)],
            &registry,
        )
        .expect("callable filter");
    assert_eq!(run.events[0].lane_id(), &LaneId::new("callable-out"));

    let mut nondeterministic = CallableFilterRegistry::default();
    nondeterministic.register(
        CallableFilterDefinition::new(
            "route-callable",
            vec![FilterRule::new(FilterPredicate::Any, FilterOp::Accept)],
        )
        .nondeterministic(),
    );
    assert!(matches!(
        callable_filter.evaluate_with_registry(
            &FilterContext::all_capabilities(),
            vec![note_event("lead", 0, 60, 80)],
            &nondeterministic,
        ),
        Err(CustomFilterError::NondeterministicCallable(name)) if name == "route-callable"
    ));

    let mut read_eval = CallableFilterRegistry::default();
    read_eval.register(
        CallableFilterDefinition::new(
            "route-callable",
            vec![FilterRule::new(FilterPredicate::Any, FilterOp::Accept)],
        )
        .with_read_eval(),
    );
    let no_read_eval = FilterContext::new(FilterCapabilitySet::new([
        FilterCapability::Callable,
        FilterCapability::Rule,
    ]));
    assert!(matches!(
        callable_filter.evaluate_with_registry(
            &no_read_eval,
            vec![note_event("lead", 0, 60, 80)],
            &read_eval,
        ),
        Err(CustomFilterError::MissingCapability(cap)) if cap == "read-eval"
    ));
}

fn run_op(op: FilterOp, input: Vec<PlayEvent>, output: FilterShape) -> crate::CustomFilterRun {
    filter_for_op(op, output)
        .evaluate(&FilterContext::all_capabilities(), input)
        .expect("filter run")
}

fn filter_for_op(op: FilterOp, output: FilterShape) -> CustomFilter {
    let capability = op.capability();
    CustomFilter::new(
        "rule-test",
        FilterShape::notes(),
        output,
        FilterCapabilitySet::rule_ops([capability]),
        DeterminismPolicy::Deterministic,
        TracePolicy::Full,
        FilterBody::Rule(vec![FilterRule::new(FilterPredicate::Any, op)]),
    )
    .expect("filter")
}

fn note_event(lane: &str, ticks: i64, midi: u8, velocity: u8) -> PlayEvent {
    PlayEvent::Note(NoteEvent {
        lane_id: LaneId::new(lane),
        time: tick(ticks),
        duration: tick(12),
        pitch: Pitch::from_midi(midi),
        velocity,
        channel: Channel::new(0).expect("channel"),
    })
}

fn control_event(lane: &str, ticks: i64, value: i64) -> PlayEvent {
    PlayEvent::Control(ControlEvent {
        lane_id: LaneId::new(lane),
        time: tick(ticks),
        control: sim_kernel::Symbol::qualified("music/control", "level"),
        value,
    })
}

fn tick(ticks: i64) -> Tick {
    Tick { ticks, tpq: 24 }
}
