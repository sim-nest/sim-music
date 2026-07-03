use sim_kernel::Symbol;
use sim_lib_music_core::{ControlEvent, LaneId, PlayEvent, Tick, TracePolicy};

use crate::filter::{
    CallableFilterRegistry, CustomFilter, CustomFilterError, CustomFilterRun, CustomFilterTrace,
    DeterminismPolicy, FilterBody, FilterCapability, FilterCapabilitySet, FilterContext, FilterOp,
    FilterRule, FilterShape, FilterTraceAction,
};

impl CustomFilter {
    /// Evaluates the filter over `events` with an empty callable registry.
    pub fn evaluate(
        &self,
        context: &FilterContext,
        events: Vec<PlayEvent>,
    ) -> Result<CustomFilterRun, CustomFilterError> {
        self.evaluate_with_registry(context, events, &CallableFilterRegistry::default())
    }

    /// Evaluates the filter over `events`, resolving callables via `registry`.
    pub fn evaluate_with_registry(
        &self,
        context: &FilterContext,
        events: Vec<PlayEvent>,
        registry: &CallableFilterRegistry,
    ) -> Result<CustomFilterRun, CustomFilterError> {
        require_capabilities(context, &self.capabilities)?;
        check_determinism(self, context)?;
        check_shape("input", &self.input, &events)?;

        let mut sequence = 0;
        let mut traces = Vec::new();
        let output = match &self.body {
            FilterBody::Rule(rules) => {
                evaluate_rules(self, rules, events, &mut traces, &mut sequence)?
            }
            FilterBody::Callable(callable) => {
                require_capability(context, FilterCapability::Callable)?;
                let definition = registry
                    .get(&callable.name)
                    .ok_or_else(|| CustomFilterError::MissingCallable(callable.name.clone()))?;
                if definition.uses_read_eval {
                    require_capability(context, FilterCapability::ReadEval)?;
                }
                if !definition.deterministic
                    && self.determinism != DeterminismPolicy::AllowNondeterministic
                {
                    return Err(CustomFilterError::NondeterministicCallable(
                        callable.name.clone(),
                    ));
                }
                require_capabilities(context, &definition.capabilities)?;
                evaluate_rules(self, &definition.rules, events, &mut traces, &mut sequence)?
            }
        };

        check_shape("output", &self.output, &output)?;
        Ok(CustomFilterRun {
            events: output,
            traces,
        })
    }
}

fn check_determinism(
    filter: &CustomFilter,
    context: &FilterContext,
) -> Result<(), CustomFilterError> {
    if filter.determinism == DeterminismPolicy::RequiresSeed && context.seed.is_none() {
        return Err(CustomFilterError::MissingSeed);
    }
    Ok(())
}

fn check_shape(
    phase: &'static str,
    shape: &FilterShape,
    events: &[PlayEvent],
) -> Result<(), CustomFilterError> {
    for event in events {
        if !shape.accepts(event) {
            return Err(CustomFilterError::ShapeMismatch {
                phase,
                kind: event.kind().wire_label(),
            });
        }
    }
    Ok(())
}

fn evaluate_rules(
    filter: &CustomFilter,
    rules: &[FilterRule],
    input: Vec<PlayEvent>,
    traces: &mut Vec<CustomFilterTrace>,
    sequence: &mut u64,
) -> Result<Vec<PlayEvent>, CustomFilterError> {
    let mut counts = vec![0_u64; rules.len()];
    let mut output = Vec::new();
    for event in input {
        let mut current = vec![event];
        for (index, rule) in rules.iter().enumerate() {
            let mut next = Vec::new();
            for event in current {
                if rule.when.matches(&event) {
                    counts[index] += 1;
                    next.extend(apply_op(
                        filter,
                        &rule.op,
                        event,
                        counts[index],
                        traces,
                        sequence,
                    )?);
                } else {
                    next.push(event);
                }
            }
            current = next;
        }
        output.extend(current);
    }
    Ok(output)
}

fn apply_op(
    filter: &CustomFilter,
    op: &FilterOp,
    event: PlayEvent,
    ordinal: u64,
    traces: &mut Vec<CustomFilterTrace>,
    sequence: &mut u64,
) -> Result<Vec<PlayEvent>, CustomFilterError> {
    match op {
        FilterOp::Accept => {
            push_trace(
                filter,
                op,
                FilterTraceAction::Accepted,
                &event,
                "",
                traces,
                sequence,
            );
            Ok(vec![event])
        }
        FilterOp::Reject => {
            push_trace(
                filter,
                op,
                FilterTraceAction::Rejected,
                &event,
                "",
                traces,
                sequence,
            );
            Ok(Vec::new())
        }
        FilterOp::Clone { copies } => {
            let mut events = vec![event.clone()];
            for _ in 0..*copies {
                push_trace(
                    filter,
                    op,
                    FilterTraceAction::Cloned,
                    &event,
                    "",
                    traces,
                    sequence,
                );
                events.push(event.clone());
            }
            Ok(events)
        }
        FilterOp::Rewrite {
            lane,
            pitch_delta,
            velocity_delta,
        } => {
            let rewritten = rewrite_event(event, lane.clone(), *pitch_delta, *velocity_delta);
            push_trace(
                filter,
                op,
                FilterTraceAction::Rewritten,
                &rewritten,
                "",
                traces,
                sequence,
            );
            Ok(vec![rewritten])
        }
        FilterOp::Route { lane } => {
            let routed = route_event(event, lane.clone());
            push_trace(
                filter,
                op,
                FilterTraceAction::Routed,
                &routed,
                "",
                traces,
                sequence,
            );
            Ok(vec![routed])
        }
        FilterOp::Annotate { message } => {
            push_trace(
                filter,
                op,
                FilterTraceAction::Annotated,
                &event,
                message,
                traces,
                sequence,
            );
            Ok(vec![event])
        }
        FilterOp::Quantize { grid_ticks } => {
            if *grid_ticks <= 0 {
                return Err(CustomFilterError::InvalidOperation(
                    "quantize grid must be positive".to_owned(),
                ));
            }
            let quantized = quantize_event(event, *grid_ticks);
            push_trace(
                filter,
                op,
                FilterTraceAction::Quantized,
                &quantized,
                "",
                traces,
                sequence,
            );
            Ok(vec![quantized])
        }
        FilterOp::Thin { keep_every } => {
            if *keep_every == 0 {
                return Err(CustomFilterError::InvalidOperation(
                    "thin keep_every must be positive".to_owned(),
                ));
            }
            let keep = (ordinal - 1).is_multiple_of(u64::from(*keep_every));
            push_trace(
                filter,
                op,
                FilterTraceAction::Thinned,
                &event,
                "",
                traces,
                sequence,
            );
            Ok(keep.then_some(event).into_iter().collect())
        }
        FilterOp::Expand { copies, step_ticks } => {
            let mut events = vec![event.clone()];
            for copy in 1..=*copies {
                let expanded = shift_event(event.clone(), *step_ticks * i64::from(copy));
                push_trace(
                    filter,
                    op,
                    FilterTraceAction::Expanded,
                    &expanded,
                    "",
                    traces,
                    sequence,
                );
                events.push(expanded);
            }
            Ok(events)
        }
        FilterOp::Sidechain { lane, control } => {
            let sidechain = sidechain_event(&event, lane.clone(), control);
            push_trace(
                filter,
                op,
                FilterTraceAction::Sidechained,
                &sidechain,
                "",
                traces,
                sequence,
            );
            Ok(vec![event, sidechain])
        }
    }
}

fn push_trace(
    filter: &CustomFilter,
    op: &FilterOp,
    action: FilterTraceAction,
    event: &PlayEvent,
    message: &str,
    traces: &mut Vec<CustomFilterTrace>,
    sequence: &mut u64,
) {
    if filter.trace == TracePolicy::Off
        || (filter.trace == TracePolicy::Diagnostics
            && !matches!(
                action,
                FilterTraceAction::Rejected | FilterTraceAction::Annotated
            ))
    {
        return;
    }
    traces.push(CustomFilterTrace {
        sequence: *sequence,
        filter_id: filter.id.clone(),
        operation: op.wire_label(),
        action,
        event: event.clone(),
        message: message.to_owned(),
    });
    *sequence += 1;
}

fn require_capabilities(
    context: &FilterContext,
    capabilities: &FilterCapabilitySet,
) -> Result<(), CustomFilterError> {
    for capability in capabilities.iter() {
        require_capability(context, capability)?;
    }
    Ok(())
}

fn require_capability(
    context: &FilterContext,
    capability: FilterCapability,
) -> Result<(), CustomFilterError> {
    if context.capabilities.contains(capability) {
        Ok(())
    } else {
        Err(CustomFilterError::MissingCapability(
            capability.wire_label().to_owned(),
        ))
    }
}

fn rewrite_event(
    event: PlayEvent,
    lane: Option<LaneId>,
    pitch_delta: i16,
    velocity_delta: i16,
) -> PlayEvent {
    let event = match lane {
        Some(lane) => route_event(event, lane),
        None => event,
    };
    match event {
        PlayEvent::Note(mut note) => {
            note.pitch = note.pitch.transpose(i32::from(pitch_delta));
            note.velocity = add_clamped_u8(note.velocity, velocity_delta);
            PlayEvent::Note(note)
        }
        PlayEvent::Pitch(mut pitch) => {
            pitch.pitch = pitch.pitch.transpose(i32::from(pitch_delta));
            PlayEvent::Pitch(pitch)
        }
        other => other,
    }
}

fn route_event(event: PlayEvent, lane_id: LaneId) -> PlayEvent {
    match event {
        PlayEvent::Note(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Note(event)
        }
        PlayEvent::Midi(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Midi(event)
        }
        PlayEvent::Pitch(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Pitch(event)
        }
        PlayEvent::Control(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Control(event)
        }
        PlayEvent::Audio(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Audio(event)
        }
        PlayEvent::Playable(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Playable(event)
        }
        PlayEvent::Performance(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Performance(event)
        }
        PlayEvent::Diagnostic(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Diagnostic(event)
        }
        PlayEvent::Trace(mut event) => {
            event.lane_id = lane_id;
            PlayEvent::Trace(event)
        }
    }
}

fn quantize_event(event: PlayEvent, grid_ticks: i64) -> PlayEvent {
    map_time(event, |time| Tick {
        ticks: ((time.ticks as f64 / grid_ticks as f64).round() as i64) * grid_ticks,
        tpq: time.tpq,
    })
}

fn shift_event(event: PlayEvent, delta_ticks: i64) -> PlayEvent {
    map_time(event, |time| Tick {
        ticks: time.ticks + delta_ticks,
        tpq: time.tpq,
    })
}

fn map_time(event: PlayEvent, map: impl Fn(Tick) -> Tick) -> PlayEvent {
    match event {
        PlayEvent::Note(mut event) => {
            event.time = map(event.time);
            PlayEvent::Note(event)
        }
        PlayEvent::Midi(mut event) => {
            event.event.time = map(event.event.time);
            PlayEvent::Midi(event)
        }
        PlayEvent::Pitch(mut event) => {
            event.time = map(event.time);
            PlayEvent::Pitch(event)
        }
        PlayEvent::Control(mut event) => {
            event.time = map(event.time);
            PlayEvent::Control(event)
        }
        PlayEvent::Audio(mut event) => {
            event.time = map(event.time);
            PlayEvent::Audio(event)
        }
        PlayEvent::Playable(mut event) => {
            event.time = map(event.time);
            PlayEvent::Playable(event)
        }
        PlayEvent::Performance(mut event) => {
            event.time = map(event.time);
            PlayEvent::Performance(event)
        }
        PlayEvent::Diagnostic(mut event) => {
            event.time = map(event.time);
            PlayEvent::Diagnostic(event)
        }
        PlayEvent::Trace(mut event) => {
            event.time = map(event.time);
            PlayEvent::Trace(event)
        }
    }
}

fn sidechain_event(event: &PlayEvent, lane_id: LaneId, control: &str) -> PlayEvent {
    let value = match event {
        PlayEvent::Note(note) => i64::from(note.velocity),
        PlayEvent::Control(control) => control.value,
        PlayEvent::Audio(audio) => i64::from(audio.frames),
        _ => 1,
    };
    PlayEvent::Control(ControlEvent {
        lane_id,
        time: event.time(),
        control: Symbol::qualified("music/sidechain", control),
        value,
    })
}

fn add_clamped_u8(value: u8, delta: i16) -> u8 {
    (i16::from(value) + delta).clamp(0, 127) as u8
}
