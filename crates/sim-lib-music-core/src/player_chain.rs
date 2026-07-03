use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{ClockDomain, StreamValue};

use crate::freeze::{context_hash, freeze_meta, stable_hash};
use crate::{
    ChainDevice, ChainPlacementPlan, ChainTraceRecord, DirectRecording, FreezeMeta, FrozenPlayable,
    FrozenPlayerChain, LaneDescriptor, LaneId, LaneKind, Music, PlacedChainDevice, PlayContext,
    PlayEvent, PlayStream, Playable, PlayableDescriptor, PlayableShape, PlayerDeviceId, PlayerMode,
    PlayerTargetDescriptor, SourceRecording, TraceAction, render_music_events, stable_event_order,
};

/// Result of rendering a chain: the output events and their trace records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainRender {
    /// Rendered output events in stable order.
    pub events: Vec<PlayEvent>,
    /// Trace records describing how each device acted.
    pub traces: Vec<ChainTraceRecord>,
}

/// A source music object plus an ordered chain of devices and an output target.
#[derive(Clone, Debug)]
pub struct PlayerChain {
    /// Symbol identifying the source.
    pub source_id: Symbol,
    /// Source music rendered at the head of the chain.
    pub source: Music,
    /// Devices applied in order to the source events.
    pub devices: Vec<ChainDevice>,
    /// Descriptor of the chain's output target.
    pub target: PlayerTargetDescriptor,
}

impl PlayerChain {
    /// Creates a chain from a source and its devices and target.
    pub fn new(
        source_id: Symbol,
        source: Music,
        devices: Vec<ChainDevice>,
        target: PlayerTargetDescriptor,
    ) -> Self {
        Self {
            source_id,
            source,
            devices,
            target,
        }
    }

    /// Returns the devices in stable order by `order` then id.
    pub fn stable_devices(&self) -> Vec<&ChainDevice> {
        let mut devices = self.devices.iter().collect::<Vec<_>>();
        devices.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
        devices
    }

    /// Builds the placement plan for the chain's devices.
    pub fn placement_plan(&self) -> ChainPlacementPlan {
        ChainPlacementPlan::new(
            self.stable_devices()
                .into_iter()
                .map(|device| PlacedChainDevice {
                    device_id: device.id.clone(),
                    placement: device.placement.clone(),
                })
                .collect(),
        )
    }

    /// Renders the source through every active device into events and traces.
    pub fn render_chain(&self, cx: &PlayContext) -> Result<ChainRender> {
        let mut events = render_music_events(&self.source, cx)?;
        let mut traces = Vec::new();
        let mut sequence = 0;
        for device in active_devices(self.stable_devices()) {
            events = process_device(device, events, cx, &mut traces, &mut sequence);
            stable_event_order(&mut events);
        }
        Ok(ChainRender { events, traces })
    }

    /// Records source metadata (hashes, seed, placement) without rendering output.
    pub fn record_source(&self, cx: &PlayContext) -> SourceRecording {
        SourceRecording::new(
            self.source_id.clone(),
            self.chain_hash(),
            context_hash(cx),
            cx.seed,
            self.placement_plan(),
        )
    }

    /// Freezes the chain and captures it as a direct recording.
    pub fn record_direct(&self, cx: &PlayContext) -> Result<DirectRecording> {
        let frozen = self.freeze_chain(cx)?;
        Ok(DirectRecording {
            meta: frozen.meta,
            events: frozen.events,
            traces: frozen.traces,
        })
    }

    /// Renders the chain and freezes it with metadata into a [`FrozenPlayerChain`].
    pub fn freeze_chain(&self, cx: &PlayContext) -> Result<FrozenPlayerChain> {
        let render = self.render_chain(cx)?;
        let meta = self.freeze_meta(cx, &render.events, &render.traces);
        Ok(FrozenPlayerChain {
            meta,
            events: render.events,
            traces: render.traces,
        })
    }

    /// Builds freeze metadata for already-rendered events and traces.
    pub fn freeze_meta(
        &self,
        cx: &PlayContext,
        events: &[PlayEvent],
        traces: &[ChainTraceRecord],
    ) -> FreezeMeta {
        freeze_meta(
            self.source_id.clone(),
            self.chain_hash(),
            cx,
            self.placement_plan(),
            events,
            traces,
        )
    }

    /// Returns a stable hash over the source id, devices, and target.
    pub fn chain_hash(&self) -> String {
        stable_hash(
            "player-chain",
            &(&self.source_id, &self.devices, &self.target),
        )
    }
}

impl Playable for PlayerChain {
    fn describe(&self) -> Result<PlayableDescriptor> {
        Ok(PlayableDescriptor {
            id: Symbol::qualified("music/player-chain", self.source_id.to_string()),
            lanes: vec![
                LaneDescriptor::new(
                    LaneId::new("chain-output"),
                    LaneKind::Note,
                    self.target.target.clone(),
                    0,
                )
                .map_err(|err| Error::Eval(err.to_string()))?,
            ],
            clock_domain: self.target.clock_domain(),
            latency_class: self.target.latency_class(),
            shape: PlayableShape::music_object(),
        })
    }

    fn render_range(&self, cx: &PlayContext) -> Result<PlayStream> {
        let render = self.render_chain(cx)?;
        let items = render
            .events
            .iter()
            .map(|event| event.to_stream_item(ClockDomain::MidiTick.symbol()))
            .collect::<Result<Vec<_>>>()?;
        let metadata = cx.stream_metadata(
            Symbol::qualified("music/play-stream", "player-chain"),
            items.len(),
        )?;
        Ok(StreamValue::pull(metadata, items))
    }

    fn freeze(&self, cx: &PlayContext) -> Result<FrozenPlayable> {
        let frozen = self.freeze_chain(cx)?;
        Ok(FrozenPlayable {
            descriptor: self.describe()?,
            events: frozen.events,
            content_hash: frozen.meta.output_hash,
        })
    }
}

fn active_devices(devices: Vec<&ChainDevice>) -> Vec<&ChainDevice> {
    let soloed = devices
        .iter()
        .any(|device| device.enabled && device.solo && !device.mute);
    devices
        .into_iter()
        .filter(|device| device.enabled && !device.mute)
        .filter(|device| !soloed || device.solo)
        .collect()
}

fn process_device(
    device: &ChainDevice,
    input: Vec<PlayEvent>,
    cx: &PlayContext,
    traces: &mut Vec<ChainTraceRecord>,
    sequence: &mut u64,
) -> Vec<PlayEvent> {
    if device.bypass {
        return input;
    }

    let generated = generated_events(device, cx);
    let mut output = match device.mode {
        PlayerMode::Through => {
            let mut output = routed_events(device, input, traces, sequence);
            trace_generated(device, &generated, traces, sequence);
            output.extend(generated);
            output
        }
        PlayerMode::Replace => {
            trace_dropped(device, input, traces, sequence);
            trace_generated(device, &generated, traces, sequence);
            generated
        }
        PlayerMode::Filter => filter_events(device, input, traces, sequence),
        PlayerMode::Sidechain => {
            trace_generated(device, &generated, traces, sequence);
            routed_events(device, input, traces, sequence)
        }
        PlayerMode::SelfClocked => {
            trace_dropped(device, input, traces, sequence);
            trace_generated(device, &generated, traces, sequence);
            generated
        }
    };
    stable_event_order(&mut output);
    output
}

fn generated_events(device: &ChainDevice, cx: &PlayContext) -> Vec<PlayEvent> {
    device
        .generated
        .iter()
        .filter_map(|event| clip_event(event, cx))
        .map(|event| route_event(device, event))
        .collect()
}

fn filter_events(
    device: &ChainDevice,
    input: Vec<PlayEvent>,
    traces: &mut Vec<ChainTraceRecord>,
    sequence: &mut u64,
) -> Vec<PlayEvent> {
    let mut output = Vec::new();
    for event in input {
        if device.filter_lanes.binary_search(event.lane_id()).is_ok() {
            push_trace(
                device,
                TraceAction::Dropped,
                event,
                "filtered",
                traces,
                sequence,
            );
        } else {
            let routed = route_event(device, event);
            let action = if device.route_lane.is_some() {
                TraceAction::Rewritten
            } else {
                TraceAction::Routed
            };
            push_trace(
                device,
                action,
                routed.clone(),
                "filtered-through",
                traces,
                sequence,
            );
            output.push(routed);
        }
    }
    output
}

fn routed_events(
    device: &ChainDevice,
    input: Vec<PlayEvent>,
    traces: &mut Vec<ChainTraceRecord>,
    sequence: &mut u64,
) -> Vec<PlayEvent> {
    input
        .into_iter()
        .map(|event| {
            let routed = route_event(device, event);
            let action = if device.route_lane.is_some() {
                TraceAction::Rewritten
            } else {
                TraceAction::Routed
            };
            push_trace(device, action, routed.clone(), "routed", traces, sequence);
            routed
        })
        .collect()
}

fn trace_generated(
    device: &ChainDevice,
    generated: &[PlayEvent],
    traces: &mut Vec<ChainTraceRecord>,
    sequence: &mut u64,
) {
    for event in generated {
        push_trace(
            device,
            TraceAction::Generated,
            event.clone(),
            "generated",
            traces,
            sequence,
        );
    }
}

fn trace_dropped(
    device: &ChainDevice,
    input: Vec<PlayEvent>,
    traces: &mut Vec<ChainTraceRecord>,
    sequence: &mut u64,
) {
    for event in input {
        push_trace(
            device,
            TraceAction::Dropped,
            event,
            "dropped",
            traces,
            sequence,
        );
    }
}

fn push_trace(
    device: &ChainDevice,
    action: TraceAction,
    event: PlayEvent,
    detail: &'static str,
    traces: &mut Vec<ChainTraceRecord>,
    sequence: &mut u64,
) {
    traces.push(ChainTraceRecord::new(
        *sequence,
        PlayerDeviceId::new(device.id.0.clone()),
        action,
        event,
        detail,
    ));
    *sequence += 1;
}

fn clip_event(event: &PlayEvent, cx: &PlayContext) -> Option<PlayEvent> {
    match event {
        PlayEvent::Note(note) => {
            let (time, duration) = cx.range.clip_span(note.time, note.duration)?;
            let mut note = note.clone();
            note.time = time;
            note.duration = duration;
            Some(PlayEvent::Note(note))
        }
        _ if cx.range.contains(event.time()) => Some(event.clone()),
        _ => None,
    }
}

fn route_event(device: &ChainDevice, event: PlayEvent) -> PlayEvent {
    let Some(lane) = &device.route_lane else {
        return event;
    };
    with_lane(event, lane.clone())
}

fn with_lane(event: PlayEvent, lane_id: LaneId) -> PlayEvent {
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
