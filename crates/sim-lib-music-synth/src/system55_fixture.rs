use sim_lib_audio_graph_core::BlockEvent;

use crate::{
    InstrumentPatch, System55, System55RenderMode, fixture::render_processor,
    system55_default_patch_id, system55_filter_bank_patch, system55_ladder_self_oscillation_patch,
    system55_oscillator_stack_patch, system55_patch_round_trip_patch, system55_recipe_path,
    system55_sequencer_patch, system55_user_patch_path,
};

/// Repository-relative path to the generated render-fixture manifest.
pub const SYSTEM55_RENDER_FIXTURE_MANIFEST_PATH: &str =
    "crates/sim-lib-music-synth/fixtures/system55/render-fixtures.toml";
/// Command that regenerates the render-fixture manifest.
pub const SYSTEM55_FIXTURE_REGENERATE_COMMAND: &str =
    "cargo run -p sim-lib-music-synth --bin system55-fixtures";
/// Stable ids for the five System 55 render fixtures, in manifest order.
pub const SYSTEM55_RENDER_FIXTURE_IDS: [&str; 5] = [
    "system55-m55-oscillator-stack-render",
    "system55-m55-ladder-self-oscillation-render",
    "system55-m55-fixed-filter-bank-render",
    "system55-m55-sequencer-driven-patch",
    "system55-m55-default-patch-round-trip",
];

/// Category of System 55 render fixture, identifying which signal path it exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System55RenderFixtureKind {
    /// Stacked oscillators summed through the mixer.
    OscillatorStack,
    /// Ladder filter driven into self-oscillation.
    LadderSelfOscillation,
    /// Fixed filter bank shaping the oscillator stack.
    FixedFilterBank,
    /// Full voice patch driven by the internal sequencer.
    SequencerDrivenPatch,
    /// Patch serialization round-trip check.
    PatchRoundTrip,
}

impl System55RenderFixtureKind {
    /// Returns the lowercase identifier string for this fixture kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OscillatorStack => "oscillator-stack",
            Self::LadderSelfOscillation => "ladder-self-oscillation",
            Self::FixedFilterBank => "fixed-filter-bank",
            Self::SequencerDrivenPatch => "sequencer-driven-patch",
            Self::PatchRoundTrip => "patch-round-trip",
        }
    }
}

/// A single deterministic System 55 render fixture: a patch, its event stream,
/// render configuration, comparison tolerance, and recorded metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct System55RenderFixture {
    /// Stable fixture id.
    pub id: String,
    /// Signal path this fixture exercises.
    pub kind: System55RenderFixtureKind,
    /// Render mode used to produce the fixture's reference output.
    pub mode: System55RenderMode,
    /// Patch rendered by the fixture.
    pub patch: InstrumentPatch,
    /// Block events fed to the patch during rendering.
    pub events: Vec<BlockEvent<'static>>,
    /// Sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Number of frames rendered.
    pub frames: usize,
    /// Number of output channels.
    pub channels: usize,
    /// Tolerance bounds for comparing rendered output against the ideal mode.
    pub tolerance: System55RenderTolerance,
    /// Recorded metadata (hashes, versions, event labels) for the fixture.
    pub metadata: System55RenderFixtureMetadata,
}

/// Metadata recorded for a render fixture, used to detect drift in the manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct System55RenderFixtureMetadata {
    /// Sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Render mode name.
    pub mode: String,
    /// Qualified id of the rendered patch.
    pub default_patch_id: String,
    /// Hash of the patch's serialized expression form.
    pub patch_hash: String,
    /// Human-readable labels for the fixture's event sequence.
    pub event_sequence: Vec<String>,
    /// Versioned identifiers of the components involved in rendering.
    pub component_versions: Vec<String>,
    /// Hash of the rendered sample trace.
    pub trace_hash: String,
}

/// Tolerance bounds for comparing a modeled render against the ideal render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55RenderTolerance {
    /// Maximum allowed absolute per-sample delta.
    pub max_abs_delta: f32,
    /// Maximum allowed mean absolute per-sample delta.
    pub mean_abs_delta: f32,
}

impl Default for System55RenderTolerance {
    fn default() -> Self {
        Self {
            max_abs_delta: 0.45,
            mean_abs_delta: 0.25,
        }
    }
}

/// Result of comparing a fixture's modeled render against its ideal render.
#[derive(Clone, Debug, PartialEq)]
pub struct System55RenderToleranceReport {
    /// Id of the fixture compared.
    pub fixture_id: String,
    /// Number of samples compared.
    pub frames: usize,
    /// Observed maximum absolute per-sample delta.
    pub max_abs_delta: f32,
    /// Observed mean absolute per-sample delta.
    pub mean_abs_delta: f32,
    /// Peak magnitude of the ideal render.
    pub ideal_peak: f32,
    /// Peak magnitude of the modeled render.
    pub rendered_peak: f32,
    /// Whether both deltas stayed within the fixture's tolerance.
    pub passed: bool,
}

/// Gate describing the render modes, fixtures, and recipe a System 55 build must
/// satisfy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct System55RenderGate {
    /// Render modes the gate covers.
    pub modes: Vec<System55RenderMode>,
    /// Fixture ids required to be present and passing.
    pub required_fixture_ids: Vec<String>,
    /// Repository path to the governing recipe.
    pub recipe_path: &'static str,
}

/// Returns the command that regenerates the render-fixture manifest.
pub fn system55_fixture_regeneration_command() -> &'static str {
    SYSTEM55_FIXTURE_REGENERATE_COMMAND
}

/// Returns the owned ids of all System 55 render fixtures.
pub fn system55_render_fixture_ids() -> Vec<String> {
    SYSTEM55_RENDER_FIXTURE_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect()
}

/// Returns the render gate covering all modes and required fixtures.
pub fn system55_render_gate() -> System55RenderGate {
    System55RenderGate {
        modes: vec![
            System55RenderMode::Ideal,
            System55RenderMode::Modeled,
            System55RenderMode::Trace,
        ],
        required_fixture_ids: system55_render_fixture_ids(),
        recipe_path: system55_recipe_path(),
    }
}

/// Builds the full set of System 55 render fixtures with their patches and events.
pub fn system55_render_fixtures() -> Vec<System55RenderFixture> {
    vec![
        render_fixture(
            SYSTEM55_RENDER_FIXTURE_IDS[0],
            System55RenderFixtureKind::OscillatorStack,
            System55RenderMode::Ideal,
            system55_oscillator_stack_patch(),
            note_sequence(0, 60, 96, 48),
            64,
        ),
        render_fixture(
            SYSTEM55_RENDER_FIXTURE_IDS[1],
            System55RenderFixtureKind::LadderSelfOscillation,
            System55RenderMode::Trace,
            system55_ladder_self_oscillation_patch(),
            Vec::new(),
            96,
        ),
        render_fixture(
            SYSTEM55_RENDER_FIXTURE_IDS[2],
            System55RenderFixtureKind::FixedFilterBank,
            System55RenderMode::Modeled,
            system55_filter_bank_patch(),
            note_sequence(0, 64, 112, 56),
            96,
        ),
        render_fixture(
            SYSTEM55_RENDER_FIXTURE_IDS[3],
            System55RenderFixtureKind::SequencerDrivenPatch,
            System55RenderMode::Trace,
            system55_sequencer_patch(),
            Vec::new(),
            128,
        ),
        render_fixture(
            SYSTEM55_RENDER_FIXTURE_IDS[4],
            System55RenderFixtureKind::PatchRoundTrip,
            System55RenderMode::Ideal,
            system55_patch_round_trip_patch(),
            note_sequence(0, 67, 100, 56),
            96,
        ),
    ]
}

/// Renders a fixture's patch in its own mode and returns the per-channel samples.
pub fn render_system55_fixture(fixture: &System55RenderFixture) -> Vec<Vec<f32>> {
    let mut system = System55::new(fixture.patch.clone(), fixture.mode);
    render_processor(
        &mut system,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    )
}

/// Renders a fixture in both its ideal and its declared mode and reports the
/// per-sample deltas against the fixture's tolerance.
pub fn system55_mode_tolerance_report(
    fixture: &System55RenderFixture,
) -> System55RenderToleranceReport {
    let mut ideal = System55::new(fixture.patch.clone(), System55RenderMode::Ideal);
    let mut rendered = System55::new(fixture.patch.clone(), fixture.mode);
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
    System55RenderToleranceReport {
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

/// Renders every fixture and serializes the full render-fixture manifest as TOML.
pub fn system55_render_fixture_manifest() -> String {
    let mut out = String::new();
    out.push_str("# Generated by `cargo run -p sim-lib-music-synth --bin system55-fixtures`.\n");
    out.push_str(&format!(
        "regenerate_command = \"{}\"\n",
        toml_escape(SYSTEM55_FIXTURE_REGENERATE_COMMAND)
    ));
    out.push_str(&format!(
        "default_patch_id = \"{}\"\n",
        system55_default_patch_id().as_qualified_str()
    ));
    out.push_str(&format!(
        "recipe_path = \"{}\"\n",
        toml_escape(system55_recipe_path())
    ));
    out.push_str(&format!(
        "user_patch_path = \"{}\"\n\n",
        toml_escape(system55_user_patch_path())
    ));

    for fixture in system55_render_fixtures() {
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
            "patch_id = \"{}\"\npatch_hash = \"{}\"\ntrace_hash = \"{}\"\n",
            fixture.metadata.default_patch_id,
            fixture.metadata.patch_hash,
            fixture.metadata.trace_hash
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
    if out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn render_fixture(
    id: &'static str,
    kind: System55RenderFixtureKind,
    mode: System55RenderMode,
    patch: InstrumentPatch,
    events: Vec<BlockEvent<'static>>,
    frames: usize,
) -> System55RenderFixture {
    let sample_rate_hz = 48_000;
    let channels = 1;
    let tolerance = System55RenderTolerance::default();
    let metadata = fixture_metadata(&patch, mode, &events, sample_rate_hz, frames, channels);
    System55RenderFixture {
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
    mode: System55RenderMode,
    events: &[BlockEvent<'static>],
    sample_rate_hz: u32,
    frames: usize,
    channels: usize,
) -> System55RenderFixtureMetadata {
    let mut system = System55::new(patch.clone(), mode);
    let trace = render_processor(&mut system, events, sample_rate_hz, frames, channels);
    System55RenderFixtureMetadata {
        sample_rate_hz,
        mode: mode.as_str().to_owned(),
        default_patch_id: patch.name.as_qualified_str(),
        patch_hash: hash_patch(patch),
        event_sequence: events.iter().copied().map(event_label).collect(),
        component_versions: component_versions(),
        trace_hash: hash_samples(&trace),
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
        "System55",
        "System55VcoDriver",
        "System55Vco",
        "System55Mixer",
        "System55LadderLpf",
        "System55Envelope",
        "System55Vca",
        "System55FixedFilterBank",
        "System55Sequencer",
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

fn hash_samples(samples: &[Vec<f32>]) -> String {
    let mut hash = Fnv64::new();
    for channel in samples {
        for sample in channel {
            hash.i64((*sample * 1_000_000.0).round() as i64);
        }
    }
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

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }

    fn finish_hex(self) -> String {
        format!("{:016x}", self.0)
    }
}
