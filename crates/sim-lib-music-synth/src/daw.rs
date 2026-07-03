use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::Graph;

use crate::{
    ComponentBackend, Dx7Patch, Ps3300RenderMode, System55RenderMode, System700RenderMode,
    dx7_component_id, dx7_voice_audio_graph, ps_3300_component_id, ps3300_audio_graph,
    ps3300_default_patch, system_55_component_id, system_700_component_id, system55_audio_graph,
    system55_default_patch, system700_audio_graph, system700_default_patch,
};

/// The cargo command that runs the DAW instrument graph render smoke test.
pub const DAW_INSTRUMENT_RENDER_SMOKE_COMMAND: &str =
    "cargo test -p sim-lib-music-synth daw_instrument_graph_nodes_render_smoke";
/// The session graph fixture names for the four DAW instrument graph kinds.
pub const DAW_INSTRUMENT_SESSION_FIXTURE_NAMES: [&str; 4] = [
    "dx7-session-graph",
    "system700-session-graph",
    "system55-session-graph",
    "ps3300-session-graph",
];
/// The recipe path for the generic instrument streaming recipe.
pub const GENERIC_INSTRUMENT_STREAM_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/streaming/generic-instrument-stream/recipe.toml";
/// The recipe path for the DX7 local streaming recipe.
pub const DX7_LOCAL_STREAM_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/streaming/dx7-local-stream/recipe.toml";
/// The recipe path for the modular local streaming recipe.
pub const MODULAR_LOCAL_STREAM_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/streaming/modular-local-stream/recipe.toml";
/// The recipe path for the all-local placement recipe.
pub const ALL_LOCAL_PLACEMENT_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/placement/all-local/recipe.toml";
/// The recipe path for the local-voice, server-side-FX placement recipe.
pub const VOICE_LOCAL_SERVER_FX_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/placement/voice-local-server-fx/recipe.toml";
/// The recipe path for the local-voice, LAN-preview placement recipe.
pub const VOICE_LOCAL_LAN_PREVIEW_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/placement/voice-local-lan-preview/recipe.toml";
/// The recipe path for the browser-wasm local placement recipe.
pub const BROWSER_WASM_LOCAL_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/placement/browser-wasm-local/recipe.toml";

const GENERIC_STREAM_IDS: [&str; 6] = [
    "stream/live/patch",
    "stream/live/midi",
    "stream/live/parameter",
    "stream/live/audio-output",
    "stream/live/preview",
    "stream/live/diagnostic",
];
const DX7_STREAM_IDS: [&str; 6] = [
    "stream/live/dx7-patch",
    "stream/live/midi",
    "stream/live/parameter",
    "stream/live/audio-output",
    "stream/live/preview",
    "stream/live/diagnostic",
];
const MODULAR_STREAM_IDS: [&str; 6] = [
    "stream/live/modular-patch",
    "stream/live/midi",
    "stream/live/parameter",
    "stream/live/audio-output",
    "stream/live/preview",
    "stream/live/diagnostic",
];
const GENERIC_STREAM_ARTIFACTS: [&str; 4] = [
    "generic-instrument-stream.report.json",
    "generic-instrument-stream.streams.l8b",
    "generic-instrument-stream.preview.sha256",
    "generic-instrument-stream.trace.jsonl",
];
const DX7_STREAM_ARTIFACTS: [&str; 4] = [
    "dx7-local-stream.report.json",
    "dx7-local-stream.streams.l8b",
    "dx7-local-stream.preview.sha256",
    "dx7-local-stream.trace.jsonl",
];
const MODULAR_STREAM_ARTIFACTS: [&str; 4] = [
    "modular-local-stream.report.json",
    "modular-local-stream.streams.l8b",
    "modular-local-stream.preview.sha256",
    "modular-local-stream.trace.jsonl",
];
const ALL_LOCAL_ARTIFACTS: [&str; 2] = [
    "all-local.placement-report.json",
    "all-local.artifacts.sha256",
];
const SERVER_FX_ARTIFACTS: [&str; 2] = [
    "voice-local-server-fx.placement-report.json",
    "voice-local-server-fx.artifacts.sha256",
];
const LAN_PREVIEW_ARTIFACTS: [&str; 2] = [
    "voice-local-lan-preview.placement-report.json",
    "voice-local-lan-preview.artifacts.sha256",
];
const BROWSER_WASM_ARTIFACTS: [&str; 2] = [
    "browser-wasm-local.placement-report.json",
    "browser-wasm-local.artifacts.sha256",
];

/// One of the instrument graph kinds the DAW can place as a session node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DawInstrumentGraphKind {
    /// The Yamaha DX7 instrument.
    Dx7,
    /// The Roland System 700 instrument.
    System700,
    /// The Moog System 55 instrument.
    System55,
    /// The Korg PS-3300 instrument.
    Ps3300,
}

/// A descriptor for a DAW instrument graph node: its kind, node id, component
/// id, session fixture, and render smoke command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DawInstrumentGraphNodeDescriptor {
    kind: DawInstrumentGraphKind,
    node_id: &'static str,
    component_id: Symbol,
    session_fixture: &'static str,
    render_smoke_command: &'static str,
}

/// A streaming recipe spec: the live stream ids, latency budget, expected
/// artifacts, and whether the recipe refuses remote hard-real-time placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstrumentStreamRecipeSpec {
    id: &'static str,
    path: &'static str,
    stream_ids: &'static [&'static str],
    latency_budget: &'static str,
    artifact_names: &'static [&'static str],
    refuses_remote_hard_realtime: bool,
}

/// A placement recipe spec: the site map distributing streams across hosts,
/// plus stream ids, latency budget, and expected artifacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstrumentPlacementRecipeSpec {
    id: &'static str,
    path: &'static str,
    site_map: &'static str,
    stream_ids: &'static [&'static str],
    latency_budget: &'static str,
    artifact_names: &'static [&'static str],
}

const INSTRUMENT_STREAM_RECIPE_SPECS: [InstrumentStreamRecipeSpec; 3] = [
    InstrumentStreamRecipeSpec {
        id: "generic-instrument-stream",
        path: GENERIC_INSTRUMENT_STREAM_RECIPE_PATH,
        stream_ids: &GENERIC_STREAM_IDS,
        latency_budget: "local sample-exact audio, buffered preview <= 128 frames",
        artifact_names: &GENERIC_STREAM_ARTIFACTS,
        refuses_remote_hard_realtime: false,
    },
    InstrumentStreamRecipeSpec {
        id: "dx7-local-stream",
        path: DX7_LOCAL_STREAM_RECIPE_PATH,
        stream_ids: &DX7_STREAM_IDS,
        latency_budget: "local sample-exact audio, remote hard-real-time refused",
        artifact_names: &DX7_STREAM_ARTIFACTS,
        refuses_remote_hard_realtime: true,
    },
    InstrumentStreamRecipeSpec {
        id: "modular-local-stream",
        path: MODULAR_LOCAL_STREAM_RECIPE_PATH,
        stream_ids: &MODULAR_STREAM_IDS,
        latency_budget: "local sample-exact audio, remote hard-real-time refused",
        artifact_names: &MODULAR_STREAM_ARTIFACTS,
        refuses_remote_hard_realtime: true,
    },
];

const INSTRUMENT_PLACEMENT_RECIPE_SPECS: [InstrumentPlacementRecipeSpec; 4] = [
    InstrumentPlacementRecipeSpec {
        id: "all-local",
        path: ALL_LOCAL_PLACEMENT_RECIPE_PATH,
        site_map: "voice=stream/site/host-callback;fx=stream/site/host-callback;preview=stream/site/host-callback",
        stream_ids: &GENERIC_STREAM_IDS,
        latency_budget: "sample-exact",
        artifact_names: &ALL_LOCAL_ARTIFACTS,
    },
    InstrumentPlacementRecipeSpec {
        id: "voice-local-server-fx",
        path: VOICE_LOCAL_SERVER_FX_RECIPE_PATH,
        site_map: "voice=stream/site/host-callback;fx=stream/site/process;preview=stream/profile/server-buffered-preview",
        stream_ids: &GENERIC_STREAM_IDS,
        latency_budget: "offline-render plus buffered-preview",
        artifact_names: &SERVER_FX_ARTIFACTS,
    },
    InstrumentPlacementRecipeSpec {
        id: "voice-local-lan-preview",
        path: VOICE_LOCAL_LAN_PREVIEW_RECIPE_PATH,
        site_map: "voice=stream/site/host-callback;preview=stream/profile/lan-buffered-audio-preview",
        stream_ids: &GENERIC_STREAM_IDS,
        latency_budget: "interactive control plus buffered-preview",
        artifact_names: &LAN_PREVIEW_ARTIFACTS,
    },
    InstrumentPlacementRecipeSpec {
        id: "browser-wasm-local",
        path: BROWSER_WASM_LOCAL_RECIPE_PATH,
        site_map: "voice=stream/site/browser-wasm;preview=stream/site/browser-wasm",
        stream_ids: &GENERIC_STREAM_IDS,
        latency_budget: "browser-local",
        artifact_names: &BROWSER_WASM_ARTIFACTS,
    },
];

impl DawInstrumentGraphKind {
    /// Returns all four instrument graph kinds.
    pub fn all() -> [Self; 4] {
        [Self::Dx7, Self::System700, Self::System55, Self::Ps3300]
    }

    /// Returns the stable lowercase token for this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dx7 => "dx7",
            Self::System700 => "system700",
            Self::System55 => "system55",
            Self::Ps3300 => "ps3300",
        }
    }

    /// Parses a kind from its lowercase token, erroring on an unknown name.
    pub fn parse_name(text: &str) -> Result<Self> {
        match text {
            "dx7" => Ok(Self::Dx7),
            "system700" => Ok(Self::System700),
            "system55" => Ok(Self::System55),
            "ps3300" => Ok(Self::Ps3300),
            _ => Err(Error::Eval(format!(
                "unknown DAW instrument graph kind: {text}"
            ))),
        }
    }
}

impl DawInstrumentGraphNodeDescriptor {
    /// Returns the instrument graph kind.
    pub fn kind(&self) -> DawInstrumentGraphKind {
        self.kind
    }

    /// Returns the session node id.
    pub fn node_id(&self) -> &'static str {
        self.node_id
    }

    /// Returns the component id for this node.
    pub fn component_id(&self) -> &Symbol {
        &self.component_id
    }

    /// Returns the session fixture name.
    pub fn session_fixture(&self) -> &'static str {
        self.session_fixture
    }

    /// Returns the render smoke test command.
    pub fn render_smoke_command(&self) -> &'static str {
        self.render_smoke_command
    }
}

impl InstrumentStreamRecipeSpec {
    /// Returns the recipe id.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// Returns the recipe path.
    pub fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the live stream ids this recipe uses.
    pub fn stream_ids(&self) -> &'static [&'static str] {
        self.stream_ids
    }

    /// Returns the human-readable latency budget.
    pub fn latency_budget(&self) -> &'static str {
        self.latency_budget
    }

    /// Returns the expected artifact names.
    pub fn artifact_names(&self) -> &'static [&'static str] {
        self.artifact_names
    }

    /// Returns whether this recipe refuses remote hard-real-time placement.
    pub fn refuses_remote_hard_realtime(&self) -> bool {
        self.refuses_remote_hard_realtime
    }
}

impl InstrumentPlacementRecipeSpec {
    /// Returns the recipe id.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// Returns the recipe path.
    pub fn path(&self) -> &'static str {
        self.path
    }

    /// Returns the site map distributing streams across hosts.
    pub fn site_map(&self) -> &'static str {
        self.site_map
    }

    /// Returns the live stream ids this recipe uses.
    pub fn stream_ids(&self) -> &'static [&'static str] {
        self.stream_ids
    }

    /// Returns the human-readable latency budget.
    pub fn latency_budget(&self) -> &'static str {
        self.latency_budget
    }

    /// Returns the expected artifact names.
    pub fn artifact_names(&self) -> &'static [&'static str] {
        self.artifact_names
    }
}

/// Returns the node descriptor for every DAW instrument graph kind.
pub fn daw_instrument_graph_node_descriptors() -> Vec<DawInstrumentGraphNodeDescriptor> {
    DawInstrumentGraphKind::all()
        .into_iter()
        .map(descriptor)
        .collect()
}

/// Returns the session node id for every DAW instrument graph kind.
pub fn daw_instrument_graph_node_ids() -> Vec<&'static str> {
    daw_instrument_graph_node_descriptors()
        .into_iter()
        .map(|descriptor| descriptor.node_id)
        .collect()
}

/// Returns the session graph fixture names for the DAW instruments.
pub fn daw_instrument_session_fixture_names() -> &'static [&'static str] {
    &DAW_INSTRUMENT_SESSION_FIXTURE_NAMES
}

/// Returns the render smoke test command for the DAW instruments.
pub fn daw_instrument_render_smoke_command() -> &'static str {
    DAW_INSTRUMENT_RENDER_SMOKE_COMMAND
}

/// Returns the streaming recipe specs shipped by this crate.
pub fn instrument_stream_recipe_specs() -> &'static [InstrumentStreamRecipeSpec] {
    &INSTRUMENT_STREAM_RECIPE_SPECS
}

/// Returns the placement recipe specs shipped by this crate.
pub fn instrument_placement_recipe_specs() -> &'static [InstrumentPlacementRecipeSpec] {
    &INSTRUMENT_PLACEMENT_RECIPE_SPECS
}

/// Returns the paths of every streaming and placement recipe.
pub fn instrument_recipe_paths() -> Vec<&'static str> {
    instrument_stream_recipe_specs()
        .iter()
        .map(InstrumentStreamRecipeSpec::path)
        .chain(
            instrument_placement_recipe_specs()
                .iter()
                .map(InstrumentPlacementRecipeSpec::path),
        )
        .collect()
}

/// Builds the audio graph for the given instrument kind from its default patch
/// and modeled (or algorithmic, for DX7) backend.
pub fn daw_instrument_audio_graph(kind: DawInstrumentGraphKind) -> Result<Graph> {
    match kind {
        DawInstrumentGraphKind::Dx7 => {
            dx7_voice_audio_graph(Dx7Patch::default(), ComponentBackend::Algorithmic)
        }
        DawInstrumentGraphKind::System700 => {
            system700_audio_graph(system700_default_patch(), System700RenderMode::Modeled)
        }
        DawInstrumentGraphKind::System55 => {
            system55_audio_graph(system55_default_patch(), System55RenderMode::Modeled)
        }
        DawInstrumentGraphKind::Ps3300 => {
            ps3300_audio_graph(ps3300_default_patch(), Ps3300RenderMode::Modeled)
        }
    }
}

fn descriptor(kind: DawInstrumentGraphKind) -> DawInstrumentGraphNodeDescriptor {
    match kind {
        DawInstrumentGraphKind::Dx7 => {
            node("dx7-voice", dx7_component_id(), "dx7-session-graph", kind)
        }
        DawInstrumentGraphKind::System700 => node(
            "system700",
            system_700_component_id(),
            "system700-session-graph",
            kind,
        ),
        DawInstrumentGraphKind::System55 => node(
            "system55",
            system_55_component_id(),
            "system55-session-graph",
            kind,
        ),
        DawInstrumentGraphKind::Ps3300 => node(
            "ps3300",
            ps_3300_component_id(),
            "ps3300-session-graph",
            kind,
        ),
    }
}

fn node(
    node_id: &'static str,
    component_id: Symbol,
    session_fixture: &'static str,
    kind: DawInstrumentGraphKind,
) -> DawInstrumentGraphNodeDescriptor {
    DawInstrumentGraphNodeDescriptor {
        kind,
        node_id,
        component_id,
        session_fixture,
        render_smoke_command: DAW_INSTRUMENT_RENDER_SMOKE_COMMAND,
    }
}
