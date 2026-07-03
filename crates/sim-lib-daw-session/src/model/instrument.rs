use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_audio_graph_core::{Patch, PatchNode};
use sim_lib_plugin_core::{PluginFormat, PluginId, PluginState};

use crate::expr_util::{
    expect_tag, expr_map, expr_string, expr_symbol, expr_vector, field, is_symbol, lookup,
    lookup_required, tag,
};

use super::{
    ClipSource, DawClip, DawSession, DawTrack, DawTrackKind, PluginChain, PluginSlot, non_empty,
    symbol,
};

const INSTRUMENT_KIND_NS: &str = "daw-instrument-kind";
const ROUTE_KIND_NS: &str = "daw-session-route-kind";

/// Suggested `cargo test` command for the instrument-session render smoke test.
pub const INSTRUMENT_SESSION_RENDER_SMOKE_COMMAND: &str =
    "cargo test -p sim-lib-daw-session instrument_session_load_render_reopen_smoke";
/// Names of the built-in instrument-session fixtures.
pub const INSTRUMENT_SESSION_FIXTURE_NAMES: [&str; 2] =
    ["instrument-session-default", "generic-synth-graph-session"];

/// Kind of modeled instrument an instance represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DawInstrumentKind {
    /// Yamaha DX7-style FM synthesizer.
    Dx7,
    /// Roland System 700-style modular synthesizer.
    System700,
    /// Roland System 55-style modular synthesizer.
    System55,
    /// Korg PS-3300-style polyphonic synthesizer.
    Ps3300,
    /// Generic synthesizer graph fixture.
    GenericSynth,
}

/// An instrument bound to a node in the session audio graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DawInstrumentInstance {
    pub(crate) id: Symbol,
    pub(crate) kind: DawInstrumentKind,
    pub(crate) graph_node_id: String,
    pub(crate) patch_fixture: String,
}

/// Kind of control route from a session source to an instrument graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DawSessionRouteKind {
    /// MIDI note/control input.
    Midi,
    /// Parameter automation lane.
    ParameterAutomation,
    /// Patch-edit surface route.
    PatchEdit,
    /// Trace/inspection route.
    Trace,
    /// Preview-render route.
    Preview,
}

/// A control route binding a session source symbol to a target graph node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DawSessionRoute {
    pub(crate) kind: DawSessionRouteKind,
    pub(crate) source: Symbol,
    pub(crate) target_node_id: String,
    pub(crate) target: Symbol,
}

impl DawInstrumentKind {
    /// Returns the stable lowercase name for this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dx7 => "dx7",
            Self::System700 => "system700",
            Self::System55 => "system55",
            Self::Ps3300 => "ps3300",
            Self::GenericSynth => "generic-synth",
        }
    }

    /// Parses an instrument kind from its stable name.
    pub fn parse_name(text: &str) -> Result<Self> {
        match text {
            "dx7" => Ok(Self::Dx7),
            "system700" => Ok(Self::System700),
            "system55" => Ok(Self::System55),
            "ps3300" => Ok(Self::Ps3300),
            "generic-synth" => Ok(Self::GenericSynth),
            _ => Err(Error::Eval(format!("unknown DAW instrument kind: {text}"))),
        }
    }

    /// Returns the namespaced symbol for this instrument kind.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified(INSTRUMENT_KIND_NS, self.as_str())
    }
}

impl DawInstrumentInstance {
    /// Creates an instrument instance, rejecting empty id, graph node id, or
    /// patch fixture.
    pub fn new(
        id: impl Into<String>,
        kind: DawInstrumentKind,
        graph_node_id: impl Into<String>,
        patch_fixture: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            id: symbol(id, "instrument id")?,
            kind,
            graph_node_id: non_empty(graph_node_id.into(), "instrument graph node id")?,
            patch_fixture: non_empty(patch_fixture.into(), "instrument patch fixture")?,
        })
    }

    /// Returns the instrument instance id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the instrument kind.
    pub fn kind(&self) -> DawInstrumentKind {
        self.kind
    }

    /// Returns the audio graph node id this instrument occupies.
    pub fn graph_node_id(&self) -> &str {
        &self.graph_node_id
    }

    /// Returns the patch fixture name backing this instrument.
    pub fn patch_fixture(&self) -> &str {
        &self.patch_fixture
    }
}

impl DawSessionRouteKind {
    /// Returns the stable lowercase name for this route kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Midi => "midi",
            Self::ParameterAutomation => "parameter-automation",
            Self::PatchEdit => "patch-edit",
            Self::Trace => "trace",
            Self::Preview => "preview",
        }
    }

    /// Parses a route kind from its stable name.
    pub fn parse_name(text: &str) -> Result<Self> {
        match text {
            "midi" => Ok(Self::Midi),
            "parameter-automation" => Ok(Self::ParameterAutomation),
            "patch-edit" => Ok(Self::PatchEdit),
            "trace" => Ok(Self::Trace),
            "preview" => Ok(Self::Preview),
            _ => Err(Error::Eval(format!(
                "unknown DAW session route kind: {text}"
            ))),
        }
    }

    /// Returns the namespaced symbol for this route kind.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified(ROUTE_KIND_NS, self.as_str())
    }
}

impl DawSessionRoute {
    /// Creates a route, rejecting an empty target node id.
    pub fn new(
        kind: DawSessionRouteKind,
        source: Symbol,
        target_node_id: impl Into<String>,
        target: Symbol,
    ) -> Result<Self> {
        Ok(Self {
            kind,
            source,
            target_node_id: non_empty(target_node_id.into(), "session route target node id")?,
            target,
        })
    }

    /// Returns the route kind.
    pub fn kind(&self) -> DawSessionRouteKind {
        self.kind
    }

    /// Returns the route source symbol.
    pub fn source(&self) -> &Symbol {
        &self.source
    }

    /// Returns the target graph node id.
    pub fn target_node_id(&self) -> &str {
        &self.target_node_id
    }

    /// Returns the route target symbol.
    pub fn target(&self) -> &Symbol {
        &self.target
    }
}

/// Returns the built-in instrument-session fixture names.
pub fn instrument_session_fixture_names() -> &'static [&'static str] {
    &INSTRUMENT_SESSION_FIXTURE_NAMES
}

/// Returns the suggested render smoke-test command.
pub fn instrument_session_render_smoke_command() -> &'static str {
    INSTRUMENT_SESSION_RENDER_SMOKE_COMMAND
}

/// Builds the default instrument-session fixture: a patched audio graph with
/// DX7, System 700, System 55, PS-3300, and generic-synth instruments plus
/// representative tracks and routes.
pub fn instrument_session_fixture() -> DawSession {
    let mut state = PluginState::new();
    state.set_param(7, 0.5);
    state.insert_data("patch-edit-target", Expr::String("dx7-voice".to_owned()));
    let slot = PluginSlot::new(
        "dx7-slot",
        PluginId::new(PluginFormat::Sim, "sim.instrument.dx7").unwrap(),
        state,
    )
    .unwrap();
    let midi_track = DawTrack::new("midi-lead", "MIDI Lead", DawTrackKind::Midi, 1)
        .unwrap()
        .with_clip(
            DawClip::new(
                "short-midi",
                0,
                96,
                ClipSource::patch_node("dx7-voice").unwrap(),
                1.0,
            )
            .unwrap(),
        )
        .with_plugin_chain(PluginChain::default().with_slot(slot));
    let preview_track = DawTrack::audio("preview-audio", "Preview Audio", 2)
        .unwrap()
        .with_clip(DawClip::constant("preview-tone", 0, 8, 0.25).unwrap());
    let mut session = DawSession::new("instrument-session", "Instrument Session", 48_000)
        .unwrap()
        .with_patch(Patch {
            nodes: vec![
                patch_node("dx7-voice", 0, 1),
                patch_node("system700", 0, 1),
                patch_node("system55", 0, 1),
                patch_node("ps3300", 0, 1),
                patch_node("generic-synth", 0, 2),
            ],
            cables: Vec::new(),
        });
    session.add_track(midi_track).unwrap();
    session.add_track(preview_track).unwrap();
    for instrument in [
        DawInstrumentInstance::new(
            "dx7-lead",
            DawInstrumentKind::Dx7,
            "dx7-voice",
            "dx7-default-patch",
        ),
        DawInstrumentInstance::new(
            "system700-panel",
            DawInstrumentKind::System700,
            "system700",
            "system700-default-patch",
        ),
        DawInstrumentInstance::new(
            "system55-cabinet",
            DawInstrumentKind::System55,
            "system55",
            "system55-default-patch",
        ),
        DawInstrumentInstance::new(
            "ps3300-panel",
            DawInstrumentKind::Ps3300,
            "ps3300",
            "ps3300-default-patch",
        ),
        DawInstrumentInstance::new(
            "generic-synth",
            DawInstrumentKind::GenericSynth,
            "generic-synth",
            "generic-synth-graph-session",
        ),
    ] {
        session
            .add_instrument_instance(instrument.unwrap())
            .unwrap();
    }
    for route in [
        route(
            DawSessionRouteKind::Midi,
            "midi-lead",
            "dx7-voice",
            "midi-in",
        ),
        route(
            DawSessionRouteKind::ParameterAutomation,
            "automation-lane",
            "dx7-voice",
            "operator-level",
        ),
        route(
            DawSessionRouteKind::PatchEdit,
            "patch-editor",
            "system700",
            "patch",
        ),
        route(
            DawSessionRouteKind::Trace,
            "trace-reader",
            "system55",
            "trace-out",
        ),
        route(
            DawSessionRouteKind::Preview,
            "preview-renderer",
            "ps3300",
            "preview",
        ),
    ] {
        session.add_route(route).unwrap();
    }
    session
}

pub(crate) fn instrument_to_expr(instrument: &DawInstrumentInstance) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("instrument-instance")),
        (field("id"), Expr::Symbol(instrument.id.clone())),
        (field("kind"), Expr::Symbol(instrument.kind.symbol())),
        (
            field("graph-node-id"),
            Expr::String(instrument.graph_node_id.clone()),
        ),
        (
            field("patch-fixture"),
            Expr::String(instrument.patch_fixture.clone()),
        ),
    ])
}

pub(crate) fn instrument_from_expr(expr: &Expr) -> Result<DawInstrumentInstance> {
    let map = expr_map(expr, "DAW instrument instance")?;
    expect_tag(map, "instrument-instance", "DAW instrument instance")?;
    Ok(DawInstrumentInstance {
        id: expr_symbol(lookup_required(map, "id")?, "instrument id")?,
        kind: kind_from_expr(lookup_required(map, "kind")?)?,
        graph_node_id: non_empty(
            expr_string(lookup_required(map, "graph-node-id")?, "graph node id")?.to_owned(),
            "instrument graph node id",
        )?,
        patch_fixture: non_empty(
            expr_string(lookup_required(map, "patch-fixture")?, "patch fixture")?.to_owned(),
            "instrument patch fixture",
        )?,
    })
}

pub(crate) fn route_to_expr(route: &DawSessionRoute) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("session-route")),
        (field("kind"), Expr::Symbol(route.kind.symbol())),
        (field("source"), Expr::Symbol(route.source.clone())),
        (
            field("target-node-id"),
            Expr::String(route.target_node_id.clone()),
        ),
        (field("target"), Expr::Symbol(route.target.clone())),
    ])
}

pub(crate) fn route_from_expr(expr: &Expr) -> Result<DawSessionRoute> {
    let map = expr_map(expr, "DAW session route")?;
    expect_tag(map, "session-route", "DAW session route")?;
    DawSessionRoute::new(
        route_kind_from_expr(lookup_required(map, "kind")?)?,
        expr_symbol(lookup_required(map, "source")?, "route source")?,
        expr_string(lookup_required(map, "target-node-id")?, "route target node")?.to_owned(),
        expr_symbol(lookup_required(map, "target")?, "route target")?,
    )
}

fn kind_from_expr(expr: &Expr) -> Result<DawInstrumentKind> {
    match expr {
        Expr::Symbol(symbol) if is_symbol(symbol, INSTRUMENT_KIND_NS, symbol.name.as_ref()) => {
            DawInstrumentKind::parse_name(symbol.name.as_ref())
        }
        Expr::String(text) => DawInstrumentKind::parse_name(text),
        _ => Err(Error::Eval("DAW instrument kind is invalid".to_owned())),
    }
}

fn route_kind_from_expr(expr: &Expr) -> Result<DawSessionRouteKind> {
    match expr {
        Expr::Symbol(symbol) if is_symbol(symbol, ROUTE_KIND_NS, symbol.name.as_ref()) => {
            DawSessionRouteKind::parse_name(symbol.name.as_ref())
        }
        Expr::String(text) => DawSessionRouteKind::parse_name(text),
        _ => Err(Error::Eval("DAW session route kind is invalid".to_owned())),
    }
}

fn patch_node(id: &str, in_channels: u16, out_channels: u16) -> PatchNode {
    PatchNode {
        id: id.to_owned(),
        in_channels,
        out_channels,
    }
}

fn route(
    kind: DawSessionRouteKind,
    source: &'static str,
    target_node_id: &'static str,
    target: &'static str,
) -> DawSessionRoute {
    DawSessionRoute::new(
        kind,
        Symbol::qualified("daw-route-source", source),
        target_node_id,
        Symbol::qualified("daw-route-target", target),
    )
    .unwrap()
}

pub(crate) fn optional_expr_vector<'a>(
    map: &'a [(Expr, Expr)],
    name: &str,
    context: &str,
) -> Result<&'a [Expr]> {
    match lookup(map, name) {
        Some(expr) => expr_vector(expr, context),
        None => Ok(&[]),
    }
}
