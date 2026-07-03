use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::Transport;
use sim_lib_audio_graph_live::LiveTransportClock;
use sim_lib_music_core::{
    Arranger, ChainPlacementPlan, LaneTarget, ParamValue, PlayContext, PlayEvent, PlayerChain,
    stable_event_order,
};
use sim_lib_stream_core::StreamEnvelope;

use crate::{
    ClipSource, DawInstrumentInstance, DawSession, DawSessionRouteKind,
    integration_stream::{
        DawPluginEventExport, event_stream_envelopes, midi_stream_envelopes, plugin_event_exports,
    },
};

/// Session-level proof that players and arrangers are bound to DAW targets.
#[derive(Clone, Debug, PartialEq)]
pub struct DawIntegratedPerformance {
    instrument_bindings: Vec<DawInstrumentBinding>,
    placement_plan: ChainPlacementPlan,
    live_schedule: DawLiveSchedule,
    events: Vec<PlayEvent>,
    stream_envelopes: Vec<StreamEnvelope>,
    midi_envelopes: Vec<StreamEnvelope>,
    plugin_events: Vec<DawPluginEventExport>,
    automation: Vec<DawPatternAutomation>,
    frozen_output_hash: String,
    trace_count: usize,
}

/// A player or arranger target resolved through the session instrument graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DawInstrumentBinding {
    instrument_id: Symbol,
    kind: &'static str,
    graph_node_id: String,
    route_kinds: Vec<DawSessionRouteKind>,
}

/// Live-preview scheduling data derived from the placement plan and clock.
#[derive(Clone, Debug, PartialEq)]
pub struct DawLiveSchedule {
    transport: Transport,
    bounded_latency_frames: u32,
    site_symbols: Vec<Symbol>,
}

/// Device automation visible to a DAW pattern lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DawPatternAutomation {
    device_id: String,
    enabled: bool,
    bypass: bool,
    params: Vec<(String, String)>,
}

/// Renders one player chain plus any arranger clips already in the session.
pub fn integrate_session_performance(
    session: &DawSession,
    chain: &PlayerChain,
    cx: &PlayContext,
) -> Result<DawIntegratedPerformance> {
    let chain_render = chain.render_chain(cx)?;
    let frozen = chain.freeze_chain(cx)?;
    let placement_plan = chain.placement_plan();
    let mut events = chain_render.events.clone();
    let mut trace_count = chain_render.traces.len();
    let mut bindings = Vec::new();

    if let Some(binding) = binding_for_target(session, &chain.target.target)? {
        bindings.push(binding);
    }
    for arranger in session_arrangers(session) {
        for placement in &arranger.placements {
            for target in &placement.targets {
                if let Some(binding) = binding_for_target(session, target)? {
                    bindings.push(binding);
                }
            }
        }
        let render = arranger.render_arrangement(cx)?;
        trace_count += render.diagnostics.len();
        events.extend(render.events);
    }
    stable_event_order(&mut events);
    dedupe_bindings(&mut bindings);

    let stream_envelopes = event_stream_envelopes(&events)?;
    let midi_envelopes = midi_stream_envelopes(&events)?;
    let plugin_events = plugin_event_exports(&events, cx)?;
    let automation = pattern_automation(chain);
    let live_schedule = live_schedule(session, &placement_plan)?;

    Ok(DawIntegratedPerformance {
        instrument_bindings: bindings,
        placement_plan,
        live_schedule,
        events,
        stream_envelopes,
        midi_envelopes,
        plugin_events,
        automation,
        frozen_output_hash: frozen.meta.output_hash,
        trace_count,
    })
}

impl DawIntegratedPerformance {
    /// Returns the player/arranger targets resolved to session instruments.
    pub fn instrument_bindings(&self) -> &[DawInstrumentBinding] {
        &self.instrument_bindings
    }

    /// Returns the device placement plan for the rendered player chain.
    pub fn placement_plan(&self) -> &ChainPlacementPlan {
        &self.placement_plan
    }

    /// Returns the live-preview scheduling data derived from the plan and clock.
    pub fn live_schedule(&self) -> &DawLiveSchedule {
        &self.live_schedule
    }

    /// Returns the ordered performance events from the chain and arrangers.
    pub fn events(&self) -> &[PlayEvent] {
        &self.events
    }

    /// Returns the event stream envelopes targeting the remote stream fabric.
    pub fn stream_envelopes(&self) -> &[StreamEnvelope] {
        &self.stream_envelopes
    }

    /// Returns the MIDI control stream envelopes targeting the LAN MIDI path.
    pub fn midi_envelopes(&self) -> &[StreamEnvelope] {
        &self.midi_envelopes
    }

    /// Returns the plugin event exports decoded from the performance events.
    pub fn plugin_events(&self) -> &[DawPluginEventExport] {
        &self.plugin_events
    }

    /// Returns the per-device pattern automation visible to DAW lanes.
    pub fn automation(&self) -> &[DawPatternAutomation] {
        &self.automation
    }

    /// Returns the deterministic hash of the frozen chain output.
    pub fn frozen_output_hash(&self) -> &str {
        &self.frozen_output_hash
    }

    /// Returns the number of diagnostic traces accumulated during integration.
    pub fn trace_count(&self) -> usize {
        self.trace_count
    }
}

impl DawInstrumentBinding {
    /// Returns the session instrument id this binding resolved to.
    pub fn instrument_id(&self) -> &Symbol {
        &self.instrument_id
    }

    /// Returns the instrument kind name (for example `"dx7"`).
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// Returns the audio graph node id the instrument occupies.
    pub fn graph_node_id(&self) -> &str {
        &self.graph_node_id
    }

    /// Returns the route kinds wired to this instrument's graph node.
    pub fn route_kinds(&self) -> &[DawSessionRouteKind] {
        &self.route_kinds
    }
}

impl DawLiveSchedule {
    /// Returns the transport snapshot computed for the live clock.
    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// Returns the bounded preview latency, in frames, across placed devices.
    pub fn bounded_latency_frames(&self) -> u32 {
        self.bounded_latency_frames
    }

    /// Returns the sorted, de-duplicated placement site symbols.
    pub fn site_symbols(&self) -> &[Symbol] {
        &self.site_symbols
    }
}

impl DawPatternAutomation {
    /// Returns the stable device id this automation applies to.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Returns whether the device is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the device is bypassed.
    pub fn bypass(&self) -> bool {
        self.bypass
    }

    /// Returns the sorted device parameters as name/value text pairs.
    pub fn params(&self) -> &[(String, String)] {
        &self.params
    }
}

fn session_arrangers(session: &DawSession) -> Vec<&Arranger> {
    session
        .tracks()
        .iter()
        .flat_map(|track| track.clips())
        .filter_map(|clip| match clip.source() {
            ClipSource::Arranger(arranger) => Some(arranger),
            _ => None,
        })
        .collect()
}

fn binding_for_target(
    session: &DawSession,
    target: &LaneTarget,
) -> Result<Option<DawInstrumentBinding>> {
    let LaneTarget::Instrument(symbol) = target else {
        return Ok(None);
    };
    let Some(instrument) = session
        .instrument_instances()
        .iter()
        .find(|instrument| instrument_matches_target(instrument, symbol))
    else {
        return Err(Error::Eval(format!(
            "DAW instrument target {symbol} is not registered in the session"
        )));
    };
    let mut route_kinds = session
        .routes()
        .iter()
        .filter(|route| route.target_node_id() == instrument.graph_node_id())
        .map(|route| route.kind())
        .collect::<Vec<_>>();
    route_kinds.sort_by_key(|kind| kind.as_str());
    route_kinds.dedup();
    Ok(Some(DawInstrumentBinding {
        instrument_id: instrument.id().clone(),
        kind: instrument.kind().as_str(),
        graph_node_id: instrument.graph_node_id().to_owned(),
        route_kinds,
    }))
}

fn instrument_matches_target(instrument: &DawInstrumentInstance, target: &Symbol) -> bool {
    instrument.id() == target
        || instrument.id().name.as_ref() == target.name.as_ref()
        || instrument.graph_node_id() == target.name.as_ref()
        || instrument.kind().as_str() == target.name.as_ref()
}

fn dedupe_bindings(bindings: &mut Vec<DawInstrumentBinding>) {
    bindings.sort_by(|left, right| {
        left.instrument_id
            .cmp(&right.instrument_id)
            .then_with(|| left.graph_node_id.cmp(&right.graph_node_id))
    });
    bindings.dedup_by(|left, right| {
        left.instrument_id == right.instrument_id && left.graph_node_id == right.graph_node_id
    });
}

fn pattern_automation(chain: &PlayerChain) -> Vec<DawPatternAutomation> {
    chain
        .stable_devices()
        .into_iter()
        .map(|device| {
            let mut params = device
                .params
                .entries
                .iter()
                .map(|(name, value)| (name.clone(), param_value_text(value)))
                .collect::<Vec<_>>();
            params.sort_by(|left, right| left.0.cmp(&right.0));
            DawPatternAutomation {
                device_id: device.id.0.clone(),
                enabled: device.enabled,
                bypass: device.bypass,
                params,
            }
        })
        .collect()
}

fn live_schedule(session: &DawSession, plan: &ChainPlacementPlan) -> Result<DawLiveSchedule> {
    let clock = LiveTransportClock::sample_frame(session.sample_rate_hz())?;
    let transport = clock.transport_at(
        session.transport().sample_pos(),
        session.transport().playing(),
    );
    let mut site_symbols = plan
        .devices
        .iter()
        .map(|device| device.placement.site.as_symbol().clone())
        .collect::<Vec<_>>();
    site_symbols.sort();
    site_symbols.dedup();
    let bounded_latency_frames = plan
        .devices
        .iter()
        .map(|device| {
            let own = device
                .placement
                .profile
                .latency()
                .frame_count()
                .min(u64::from(u32::MAX));
            let floor = if device.placement.profile.realtime_pin() {
                128
            } else {
                512
            };
            (own as u32).max(floor)
        })
        .max()
        .unwrap_or(128);
    Ok(DawLiveSchedule {
        transport,
        bounded_latency_frames,
        site_symbols,
    })
}

fn param_value_text(value: &ParamValue) -> String {
    match value {
        ParamValue::Bool(value) => value.to_string(),
        ParamValue::I64(value) => value.to_string(),
        ParamValue::Text(value) => value.clone(),
        ParamValue::Symbol(value) => value.to_string(),
    }
}
