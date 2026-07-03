use sim_lib_audio_graph_core::BlockEvent;

use crate::{
    InstrumentPatch, Ps3300, Ps3300PatchProfile, Ps3300RenderMode, fixture::render_processor,
    ps3300_default_patch_id, ps3300_one_cell_patch, ps3300_one_section_chord_patch,
    ps3300_patch_round_trip_patch, ps3300_recipe_path, ps3300_resonator_sweep_patch,
    ps3300_three_section_stack_patch, ps3300_user_patch_path,
};

/// Repository-relative path to the generated PS-3300 render-fixture manifest.
pub const PS3300_RENDER_FIXTURE_MANIFEST_PATH: &str =
    "crates/sim-lib-music-synth/fixtures/ps3300/render-fixtures.toml";
/// Shell command that regenerates the PS-3300 render fixtures and manifest.
pub const PS3300_FIXTURE_REGENERATE_COMMAND: &str =
    "cargo run -p sim-lib-music-synth --bin ps3300-fixtures";
/// Stable ids of the five PS-3300 render fixtures, in manifest order.
pub const PS3300_RENDER_FIXTURE_IDS: [&str; 5] = [
    "ps3300-ps3-one-cell-render",
    "ps3300-ps3-one-section-chord-render",
    "ps3300-ps3-resonator-sweep-render",
    "ps3300-ps3-three-section-stack-render",
    "ps3300-default-patch-round-trip",
];

/// Which PS-3300 render scenario a fixture exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300RenderFixtureKind {
    /// Single voice cell rendering one note.
    OneCell,
    /// One section playing a chord.
    OneSectionChord,
    /// Resonator formant sweep.
    ResonatorSweep,
    /// All three sections stacked into a chord.
    ThreeSectionStack,
    /// Default patch exercised for serialization round-trip.
    PatchRoundTrip,
}

impl Ps3300RenderFixtureKind {
    /// Returns the stable kebab-case identifier string for this fixture kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneCell => "one-cell",
            Self::OneSectionChord => "one-section-chord",
            Self::ResonatorSweep => "resonator-sweep",
            Self::ThreeSectionStack => "three-section-stack",
            Self::PatchRoundTrip => "patch-round-trip",
        }
    }
}

/// A complete PS-3300 render fixture: patch, events, render config, and tolerances.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300RenderFixture {
    /// Stable fixture id.
    pub id: String,
    /// Scenario kind exercised by the fixture.
    pub kind: Ps3300RenderFixtureKind,
    /// Render mode used to produce the fixture's reference output.
    pub mode: Ps3300RenderMode,
    /// Patch rendered by the fixture.
    pub patch: InstrumentPatch,
    /// Block events (note on/off) fed during rendering.
    pub events: Vec<BlockEvent<'static>>,
    /// Sample rate in hertz for the render.
    pub sample_rate_hz: u32,
    /// Number of audio frames rendered.
    pub frames: usize,
    /// Number of output channels rendered.
    pub channels: usize,
    /// Allowed delta tolerances between ideal and mode-rendered output.
    pub tolerance: Ps3300RenderTolerance,
    /// Recorded metadata (hashes, versions, event labels) for the fixture.
    pub metadata: Ps3300RenderFixtureMetadata,
}

/// Recorded metadata captured alongside a PS-3300 render fixture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ps3300RenderFixtureMetadata {
    /// Sample rate in hertz used for the recorded render.
    pub sample_rate_hz: u32,
    /// Render mode string the fixture was captured in.
    pub mode: String,
    /// Qualified id of the patch that was rendered.
    pub default_patch_id: String,
    /// FNV-64 hash of the patch's serialized expression.
    pub patch_hash: String,
    /// Human-readable labels of each block event, in order.
    pub event_sequence: Vec<String>,
    /// Versioned identifiers of the components involved in the render.
    pub component_versions: Vec<String>,
    /// FNV-64 hash of the rendered sample trace.
    pub trace_hash: String,
}

/// Per-fixture acceptance tolerances comparing modeled output against the ideal render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300RenderTolerance {
    /// Maximum permitted absolute sample delta.
    pub max_abs_delta: f32,
    /// Maximum permitted mean absolute sample delta.
    pub mean_abs_delta: f32,
}

impl Default for Ps3300RenderTolerance {
    fn default() -> Self {
        Self {
            max_abs_delta: 0.5,
            mean_abs_delta: 0.28,
        }
    }
}

/// Outcome of comparing a fixture's modeled render against its ideal render.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300RenderToleranceReport {
    /// Id of the fixture that was checked.
    pub fixture_id: String,
    /// Number of samples compared across all channels.
    pub frames: usize,
    /// Observed maximum absolute sample delta.
    pub max_abs_delta: f32,
    /// Observed mean absolute sample delta.
    pub mean_abs_delta: f32,
    /// Peak absolute amplitude of the ideal render.
    pub ideal_peak: f32,
    /// Peak absolute amplitude of the mode-rendered output.
    pub rendered_peak: f32,
    /// Whether both observed deltas fall within the fixture tolerance.
    pub passed: bool,
}

/// Acceptance gate describing which render modes and fixtures must pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ps3300RenderGate {
    /// Render modes the gate covers.
    pub modes: Vec<Ps3300RenderMode>,
    /// Fixture ids that must be present and pass.
    pub required_fixture_ids: Vec<String>,
    /// Repository-relative recipe path backing the gate.
    pub recipe_path: &'static str,
}

/// Returns the command string that regenerates the PS-3300 fixtures.
pub fn ps3300_fixture_regeneration_command() -> &'static str {
    PS3300_FIXTURE_REGENERATE_COMMAND
}

/// Returns the render-fixture ids as owned strings.
pub fn ps3300_render_fixture_ids() -> Vec<String> {
    PS3300_RENDER_FIXTURE_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect()
}

/// Returns the PS-3300 render gate covering all modes and required fixtures.
pub fn ps3300_render_gate() -> Ps3300RenderGate {
    Ps3300RenderGate {
        modes: vec![
            Ps3300RenderMode::Ideal,
            Ps3300RenderMode::Modeled,
            Ps3300RenderMode::Trace,
        ],
        required_fixture_ids: ps3300_render_fixture_ids(),
        recipe_path: ps3300_recipe_path(),
    }
}

/// Builds the full set of PS-3300 render fixtures with patches and event sequences.
pub fn ps3300_render_fixtures() -> Vec<Ps3300RenderFixture> {
    vec![
        render_fixture(
            PS3300_RENDER_FIXTURE_IDS[0],
            Ps3300RenderFixtureKind::OneCell,
            Ps3300RenderMode::Ideal,
            ps3300_one_cell_patch(),
            note_sequence(0, 48, 96, 56),
            96,
        ),
        render_fixture(
            PS3300_RENDER_FIXTURE_IDS[1],
            Ps3300RenderFixtureKind::OneSectionChord,
            Ps3300RenderMode::Modeled,
            ps3300_one_section_chord_patch(),
            chord_sequence(0, &[48, 55, 60], 104, 72),
            128,
        ),
        render_fixture(
            PS3300_RENDER_FIXTURE_IDS[2],
            Ps3300RenderFixtureKind::ResonatorSweep,
            Ps3300RenderMode::Trace,
            ps3300_resonator_sweep_patch(),
            note_sequence(0, 52, 112, 96),
            144,
        ),
        render_fixture(
            PS3300_RENDER_FIXTURE_IDS[3],
            Ps3300RenderFixtureKind::ThreeSectionStack,
            Ps3300RenderMode::Modeled,
            ps3300_three_section_stack_patch(),
            chord_sequence(0, &[43, 50, 55, 62], 110, 96),
            160,
        ),
        render_fixture(
            PS3300_RENDER_FIXTURE_IDS[4],
            Ps3300RenderFixtureKind::PatchRoundTrip,
            Ps3300RenderMode::Ideal,
            ps3300_patch_round_trip_patch(),
            note_sequence(0, 64, 100, 72),
            128,
        ),
    ]
}

/// Renders a fixture's patch with its events and returns per-channel samples.
pub fn render_ps3300_fixture(fixture: &Ps3300RenderFixture) -> Vec<Vec<f32>> {
    let mut ps3300 = Ps3300::new(fixture.patch.clone(), fixture.mode);
    render_processor(
        &mut ps3300,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    )
}

/// Renders a fixture in ideal and target modes and reports the sample deltas.
pub fn ps3300_mode_tolerance_report(fixture: &Ps3300RenderFixture) -> Ps3300RenderToleranceReport {
    let mut ideal = Ps3300::new(fixture.patch.clone(), Ps3300RenderMode::Ideal);
    let mut rendered = Ps3300::new(fixture.patch.clone(), fixture.mode);
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
    Ps3300RenderToleranceReport {
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

/// Renders every fixture and serializes the render-fixture manifest as TOML text.
pub fn ps3300_render_fixture_manifest() -> String {
    let mut out = String::new();
    out.push_str("# Generated by `cargo run -p sim-lib-music-synth --bin ps3300-fixtures`.\n");
    out.push_str(&format!(
        "regenerate_command = \"{}\"\n",
        toml_escape(PS3300_FIXTURE_REGENERATE_COMMAND)
    ));
    out.push_str(&format!(
        "default_patch_id = \"{}\"\n",
        ps3300_default_patch_id().as_qualified_str()
    ));
    out.push_str(&format!(
        "recipe_path = \"{}\"\n",
        toml_escape(ps3300_recipe_path())
    ));
    out.push_str(&format!(
        "user_patch_path = \"{}\"\n\n",
        toml_escape(ps3300_user_patch_path())
    ));

    for fixture in ps3300_render_fixtures() {
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
    kind: Ps3300RenderFixtureKind,
    mode: Ps3300RenderMode,
    patch: InstrumentPatch,
    events: Vec<BlockEvent<'static>>,
    frames: usize,
) -> Ps3300RenderFixture {
    let sample_rate_hz = 48_000;
    let channels = 1;
    let tolerance = Ps3300RenderTolerance::default();
    let metadata = fixture_metadata(&patch, mode, &events, sample_rate_hz, frames, channels);
    Ps3300RenderFixture {
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
    mode: Ps3300RenderMode,
    events: &[BlockEvent<'static>],
    sample_rate_hz: u32,
    frames: usize,
    channels: usize,
) -> Ps3300RenderFixtureMetadata {
    let mut ps3300 = Ps3300::new(patch.clone(), mode);
    let trace = render_processor(&mut ps3300, events, sample_rate_hz, frames, channels);
    Ps3300RenderFixtureMetadata {
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

fn chord_sequence(
    offset: u32,
    keys: &[u8],
    velocity: u8,
    note_off_offset: u32,
) -> Vec<BlockEvent<'static>> {
    let mut events = keys
        .iter()
        .map(|key| BlockEvent::NoteOn {
            offset,
            channel: 0,
            key: *key,
            velocity: f32::from(velocity) / 127.0,
        })
        .collect::<Vec<_>>();
    events.extend(keys.iter().map(|key| BlockEvent::NoteOff {
        offset: note_off_offset,
        channel: 0,
        key: *key,
        velocity: 0.0,
    }));
    events
}

fn component_versions() -> Vec<String> {
    [
        "sim-lib-music-synth",
        "Ps3300",
        "Ps3300KeyboardController",
        "Ps3300PinMatrix",
        "Ps3300SectionGenerator",
        "Ps3300TripleResonator",
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

/// Returns the five patch profiles corresponding to the render fixtures.
pub fn ps3300_fixture_profiles() -> [Ps3300PatchProfile; 5] {
    [
        Ps3300PatchProfile::OneCell,
        Ps3300PatchProfile::OneSectionChord,
        Ps3300PatchProfile::ResonatorSweep,
        Ps3300PatchProfile::ThreeSectionStack,
        Ps3300PatchProfile::PatchRoundTrip,
    ]
}
