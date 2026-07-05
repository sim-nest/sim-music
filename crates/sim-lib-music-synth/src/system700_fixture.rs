use sim_lib_audio_graph_core::BlockEvent;

use crate::{
    InstrumentPatch, System700, System700RenderMode, fixture::render_processor,
    system700_default_patch, system700_default_patch_id, system700_patch_round_trip_patch,
    system700_sequencer_patch, system700_single_module_patch, system700_two_module_patch,
    system700_user_patch_path,
};

/// Repository path of the generated System 700 render-fixture manifest.
pub const SYSTEM700_RENDER_FIXTURE_MANIFEST_PATH: &str =
    "crates/sim-lib-music-synth/fixtures/system700/render-fixtures.toml";
/// Command that regenerates the System 700 render fixtures.
pub const SYSTEM700_FIXTURE_REGENERATE_COMMAND: &str =
    "cargo run -p xtask -- music-fixtures system700";
/// Repository path of the System 700 synthetic main-console recipe.
pub const SYSTEM700_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/system700/synthetic-main-console/recipe.toml";

/// Stable ids of the System 700 render fixtures, in manifest order.
pub const SYSTEM700_RENDER_FIXTURE_IDS: [&str; 5] = [
    "system700-r700-single-module-render",
    "system700-r700-two-module-patch-render",
    "system700-default-main-console-voice",
    "system700-sequencer-driven-patch",
    "system700-default-patch-round-trip",
];

/// The patch shape exercised by a System 700 render fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System700RenderFixtureKind {
    /// A single-module (lone VCO) patch.
    SingleModule,
    /// A two-module (VCO into VCF) patch.
    TwoModulePatch,
    /// The default-voice main-console patch.
    DefaultVoice,
    /// The sequencer-driven main-console patch.
    SequencerDrivenPatch,
    /// The patch round-trip main-console patch.
    PatchRoundTrip,
}

impl System700RenderFixtureKind {
    /// Returns the stable string name of this fixture kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SingleModule => "single-module",
            Self::TwoModulePatch => "two-module-patch",
            Self::DefaultVoice => "default-voice",
            Self::SequencerDrivenPatch => "sequencer-driven-patch",
            Self::PatchRoundTrip => "patch-round-trip",
        }
    }
}

/// A complete, reproducible System 700 render scenario: a patch, event stream,
/// render settings, and the expected metadata and tolerances.
#[derive(Clone, Debug, PartialEq)]
pub struct System700RenderFixture {
    /// Stable fixture id.
    pub id: String,
    /// The patch shape this fixture exercises.
    pub kind: System700RenderFixtureKind,
    /// Render mode used when rendering the fixture.
    pub mode: System700RenderMode,
    /// The instrument patch to render.
    pub patch: InstrumentPatch,
    /// Event stream fed during rendering.
    pub events: Vec<BlockEvent<'static>>,
    /// Sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Number of frames to render.
    pub frames: usize,
    /// Number of output channels.
    pub channels: usize,
    /// Acceptance tolerances comparing this mode against the ideal render.
    pub tolerance: System700RenderTolerance,
    /// Captured metadata describing the expected render.
    pub metadata: System700RenderFixtureMetadata,
}

/// Captured metadata that pins a System 700 render fixture to a known result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct System700RenderFixtureMetadata {
    /// Sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Render mode name.
    pub mode: String,
    /// Qualified id of the patch.
    pub default_patch_id: String,
    /// Hash of the patch expression.
    pub patch_hash: String,
    /// Human-readable labels of the event sequence.
    pub event_sequence: Vec<String>,
    /// Version strings of the components used.
    pub component_versions: Vec<String>,
}

/// Acceptance tolerances for comparing a rendered mode against the ideal render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700RenderTolerance {
    /// Maximum permitted absolute per-sample delta.
    pub max_abs_delta: f32,
    /// Maximum permitted mean absolute per-sample delta.
    pub mean_abs_delta: f32,
}

impl Default for System700RenderTolerance {
    fn default() -> Self {
        Self {
            max_abs_delta: 0.45,
            mean_abs_delta: 0.25,
        }
    }
}

/// Result of comparing a fixture's rendered mode against the ideal render.
#[derive(Clone, Debug, PartialEq)]
pub struct System700RenderToleranceReport {
    /// Id of the fixture compared.
    pub fixture_id: String,
    /// Number of sample comparisons performed.
    pub frames: usize,
    /// Largest absolute per-sample delta observed.
    pub max_abs_delta: f32,
    /// Mean absolute per-sample delta observed.
    pub mean_abs_delta: f32,
    /// Peak magnitude of the ideal render.
    pub ideal_peak: f32,
    /// Peak magnitude of the rendered mode.
    pub rendered_peak: f32,
    /// Whether both deltas fall within the fixture's tolerances.
    pub passed: bool,
}

/// The gate that the System 700 render fixtures must satisfy: the modes, the
/// required fixture ids, and the recipe path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct System700RenderGate {
    /// Render modes the gate covers.
    pub modes: Vec<System700RenderMode>,
    /// Fixture ids required to be present.
    pub required_fixture_ids: Vec<String>,
    /// Repository path of the governing recipe.
    pub recipe_path: &'static str,
}

/// Returns the command that regenerates the System 700 render fixtures.
pub fn system700_fixture_regeneration_command() -> &'static str {
    SYSTEM700_FIXTURE_REGENERATE_COMMAND
}

/// Returns the repository path of the System 700 main-console recipe.
pub fn system700_recipe_path() -> &'static str {
    SYSTEM700_RECIPE_PATH
}

/// Returns the ids of every System 700 render fixture.
pub fn system700_render_fixture_ids() -> Vec<String> {
    SYSTEM700_RENDER_FIXTURE_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect()
}

/// Returns the [`System700RenderGate`] covering all modes and required fixtures.
pub fn system700_render_gate() -> System700RenderGate {
    System700RenderGate {
        modes: vec![
            System700RenderMode::Ideal,
            System700RenderMode::Modeled,
            System700RenderMode::Trace,
        ],
        required_fixture_ids: system700_render_fixture_ids(),
        recipe_path: SYSTEM700_RECIPE_PATH,
    }
}

/// Builds the full set of System 700 render fixtures.
pub fn system700_render_fixtures() -> Vec<System700RenderFixture> {
    vec![
        render_fixture(
            SYSTEM700_RENDER_FIXTURE_IDS[0],
            System700RenderFixtureKind::SingleModule,
            System700RenderMode::Ideal,
            system700_single_module_patch(),
            note_sequence(0, 60, 96, 48),
            64,
        ),
        render_fixture(
            SYSTEM700_RENDER_FIXTURE_IDS[1],
            System700RenderFixtureKind::TwoModulePatch,
            System700RenderMode::Modeled,
            system700_two_module_patch(),
            note_sequence(0, 62, 96, 48),
            64,
        ),
        render_fixture(
            SYSTEM700_RENDER_FIXTURE_IDS[2],
            System700RenderFixtureKind::DefaultVoice,
            System700RenderMode::Modeled,
            system700_default_patch(),
            note_sequence(0, 64, 112, 56),
            96,
        ),
        render_fixture(
            SYSTEM700_RENDER_FIXTURE_IDS[3],
            System700RenderFixtureKind::SequencerDrivenPatch,
            System700RenderMode::Trace,
            system700_sequencer_patch(),
            Vec::new(),
            128,
        ),
        render_fixture(
            SYSTEM700_RENDER_FIXTURE_IDS[4],
            System700RenderFixtureKind::PatchRoundTrip,
            System700RenderMode::Ideal,
            system700_patch_round_trip_patch(),
            note_sequence(0, 67, 100, 56),
            96,
        ),
    ]
}

/// Renders `fixture` through a fresh [`System700`] and returns the per-channel
/// sample buffers.
pub fn render_system700_fixture(fixture: &System700RenderFixture) -> Vec<Vec<f32>> {
    let mut system = System700::new(fixture.patch.clone(), fixture.mode);
    render_processor(
        &mut system,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    )
}

/// Renders `fixture` in both ideal and its configured mode and returns a
/// [`System700RenderToleranceReport`] comparing the two.
pub fn system700_mode_tolerance_report(
    fixture: &System700RenderFixture,
) -> System700RenderToleranceReport {
    let mut ideal = System700::new(fixture.patch.clone(), System700RenderMode::Ideal);
    let mut rendered = System700::new(fixture.patch.clone(), fixture.mode);
    let left = render_processor(
        &mut ideal,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    );
    let right = render_processor(
        &mut rendered,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    );
    let mut max_abs_delta = 0.0_f32;
    let mut sum_abs_delta = 0.0_f32;
    let mut frames = 0_usize;
    for (left, right) in left.iter().zip(&right) {
        for (left, right) in left.iter().zip(right) {
            let delta = (left - right).abs();
            max_abs_delta = max_abs_delta.max(delta);
            sum_abs_delta += delta;
            frames += 1;
        }
    }
    let mean_abs_delta = if frames == 0 {
        0.0
    } else {
        sum_abs_delta / frames as f32
    };
    System700RenderToleranceReport {
        fixture_id: fixture.id.clone(),
        frames,
        max_abs_delta,
        mean_abs_delta,
        ideal_peak: peak(&left),
        rendered_peak: peak(&right),
        passed: max_abs_delta <= fixture.tolerance.max_abs_delta
            && mean_abs_delta <= fixture.tolerance.mean_abs_delta,
    }
}

/// Renders every fixture and serializes the System 700 render-fixture manifest
/// as TOML.
pub fn system700_render_fixture_manifest() -> String {
    let mut out = String::new();
    out.push_str("# Generated by `cargo run -p xtask -- music-fixtures system700`.\n");
    out.push_str(&format!(
        "regenerate_command = \"{}\"\n",
        toml_escape(SYSTEM700_FIXTURE_REGENERATE_COMMAND)
    ));
    out.push_str(&format!(
        "default_patch_id = \"{}\"\n",
        system700_default_patch_id().as_qualified_str()
    ));
    out.push_str(&format!(
        "recipe_path = \"{}\"\n",
        toml_escape(SYSTEM700_RECIPE_PATH)
    ));
    out.push_str(&format!(
        "user_patch_path = \"{}\"\n\n",
        toml_escape(system700_user_patch_path())
    ));

    for fixture in system700_render_fixtures() {
        out.push_str("[[fixture]]\n");
        out.push_str(&format!("id = \"{}\"\n", toml_escape(&fixture.id)));
        out.push_str(&format!(
            "kind = \"{}\"\n",
            toml_escape(fixture.kind.as_str())
        ));
        out.push_str(&format!(
            "mode = \"{}\"\nsample_rate_hz = {}\nframes = {}\nchannels = {}\n",
            fixture.mode.as_str(),
            fixture.sample_rate_hz,
            fixture.frames,
            fixture.channels
        ));
        out.push_str(&format!(
            "patch_id = \"{}\"\npatch_hash = \"{}\"\n",
            fixture.metadata.default_patch_id, fixture.metadata.patch_hash
        ));
        out.push_str(&format!(
            "event_sequence = [{}]\n",
            toml_array(&fixture.metadata.event_sequence)
        ));
        out.push_str(&format!(
            "component_versions = [{}]\n",
            toml_array(&fixture.metadata.component_versions)
        ));
        out.push_str(&format!(
            "max_abs_delta = {:.3}\nmean_abs_delta = {:.3}\n\n",
            fixture.tolerance.max_abs_delta, fixture.tolerance.mean_abs_delta
        ));
    }
    out
}

fn render_fixture(
    id: &'static str,
    kind: System700RenderFixtureKind,
    mode: System700RenderMode,
    patch: InstrumentPatch,
    events: Vec<BlockEvent<'static>>,
    frames: usize,
) -> System700RenderFixture {
    let sample_rate_hz = 48_000;
    let channels = 1;
    let tolerance = System700RenderTolerance::default();
    let metadata = fixture_metadata(&patch, mode, &events, sample_rate_hz);
    System700RenderFixture {
        id: id.to_owned(),
        kind,
        mode,
        patch,
        events,
        sample_rate_hz,
        frames,
        channels,
        tolerance,
        metadata,
    }
}

fn fixture_metadata(
    patch: &InstrumentPatch,
    mode: System700RenderMode,
    events: &[BlockEvent<'static>],
    sample_rate_hz: u32,
) -> System700RenderFixtureMetadata {
    System700RenderFixtureMetadata {
        sample_rate_hz,
        mode: mode.as_str().to_owned(),
        default_patch_id: patch.name.as_qualified_str(),
        patch_hash: hash_patch(patch),
        event_sequence: events.iter().copied().map(event_label).collect(),
        component_versions: component_versions(),
    }
}

fn note_sequence(
    offset: u32,
    key: u8,
    velocity: u8,
    note_off_offset: u32,
) -> Vec<BlockEvent<'static>> {
    vec![
        BlockEvent::NoteOn {
            offset,
            channel: 0,
            key,
            velocity: f32::from(velocity) / 127.0,
        },
        BlockEvent::NoteOff {
            offset: note_off_offset,
            channel: 0,
            key,
            velocity: 0.0,
        },
    ]
}

fn component_versions() -> Vec<String> {
    [
        "sim-lib-music-synth",
        "System700",
        "System700Vco",
        "System700Vcf",
        "System700Vca",
        "System700Sequencer",
    ]
    .into_iter()
    .map(|component| format!("{component}@{}", env!("CARGO_PKG_VERSION")))
    .collect()
}

fn event_label(event: BlockEvent<'_>) -> String {
    match event {
        BlockEvent::Midi { offset, bytes, len } => {
            format!(
                "offset:{offset}:midi:{:02x?}:len:{len}",
                &bytes[..usize::from(len)]
            )
        }
        BlockEvent::MidiLong { offset, bytes } => {
            format!("offset:{offset}:midi-long:{}-bytes", bytes.len())
        }
        BlockEvent::ParamSet {
            offset,
            param,
            value,
        } => format!("offset:{offset}:param:{param}:value:{value:.3}"),
        BlockEvent::NoteOn {
            offset,
            channel,
            key,
            velocity,
        } => format!("offset:{offset}:note-on:ch{channel}:key{key}:vel{velocity:.3}"),
        BlockEvent::NoteOff {
            offset,
            channel,
            key,
            velocity,
        } => format!("offset:{offset}:note-off:ch{channel}:key{key}:vel{velocity:.3}"),
    }
}

fn hash_patch(patch: &InstrumentPatch) -> String {
    let mut hash = Fnv64::new();
    hash.bytes(format!("{:?}", patch.to_expr()).as_bytes());
    hash.finish_hex()
}

fn peak(samples: &[Vec<f32>]) -> f32 {
    samples
        .iter()
        .flat_map(|channel| channel.iter())
        .map(|sample| sample.abs())
        .fold(0.0, f32::max)
}

fn toml_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("\"{}\"", toml_escape(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

struct Fnv64(u64);

impl Fnv64 {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.u8(*byte);
        }
    }

    fn u8(&mut self, value: u8) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn finish_hex(self) -> String {
        format!("{:016x}", self.0)
    }
}
