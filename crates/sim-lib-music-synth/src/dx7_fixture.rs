use sim_lib_audio_graph_core::BlockEvent;

use crate::{
    ComponentBackend, DX7_ALGORITHM_COUNT, DX7_OPERATOR_COUNT, Dx7Lfo, Dx7Patch, Dx7PatchOperator,
    Dx7RawPatch, Dx7Voice, fixture::render_processor,
};

/// Repository-relative path of the generated DX7 render-fixture manifest.
pub const DX7_RENDER_FIXTURE_MANIFEST_PATH: &str =
    "crates/sim-lib-music-synth/fixtures/dx7/render-fixtures.toml";
/// Cargo command that regenerates the DX7 render fixtures and their manifest.
pub const DX7_FIXTURE_REGENERATE_COMMAND: &str =
    "cargo run -p sim-lib-music-synth --bin dx7-fixtures";

/// Categorizes a DX7 render fixture by the synthesis scenario it exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dx7RenderFixtureKind {
    /// A single sounding operator with all others silenced.
    SingleOperator,
    /// Two operators wired as one modulator driving one carrier.
    TwoOperatorModulation,
    /// A full six-operator patch on the numbered DX7 algorithm.
    Algorithm(u8),
    /// An envelope gate/release scenario with note-on and note-off.
    Envelope,
}

impl Dx7RenderFixtureKind {
    /// Returns the stable manifest slug for this fixture kind.
    pub fn as_str(self) -> String {
        match self {
            Self::SingleOperator => "single-op".to_owned(),
            Self::TwoOperatorModulation => "two-op-modulation".to_owned(),
            Self::Algorithm(id) => format!("algorithm-{id:02}"),
            Self::Envelope => "envelope".to_owned(),
        }
    }
}

/// A reproducible DX7 render case: a patch, an event sequence, render
/// dimensions, comparison tolerances, and recorded metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7RenderFixture {
    /// Stable identifier used in the manifest and across reports.
    pub id: String,
    /// Scenario category this fixture exercises.
    pub kind: Dx7RenderFixtureKind,
    /// Patch rendered by the fixture.
    pub patch: Dx7Patch,
    /// Block events (note on/off, MIDI) driving the render.
    pub events: Vec<BlockEvent<'static>>,
    /// Render sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Number of frames rendered.
    pub frames: usize,
    /// Number of output channels.
    pub channels: usize,
    /// Allowed delta between the algorithmic and modeled renderers.
    pub tolerance: Dx7RenderTolerance,
    /// Recorded metadata (hashes, MIDI labels, component versions).
    pub metadata: Dx7RenderFixtureMetadata,
}

/// Provenance metadata recorded alongside a [`Dx7RenderFixture`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7RenderFixtureMetadata {
    /// Sample rate the trace was captured at, in hertz.
    pub sample_rate_hz: u32,
    /// Renderer backend label used to produce the reference trace.
    pub mode: String,
    /// FNV-64 hex hash of the patch contents.
    pub patch_hash: String,
    /// Human-readable labels for each driving event, in order.
    pub midi_sequence: Vec<String>,
    /// Versioned component identifiers contributing to the trace.
    pub component_versions: Vec<String>,
}

/// Per-fixture sample-error budget for comparing two renderer backends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dx7RenderTolerance {
    /// Maximum allowed absolute per-sample delta.
    pub max_abs_delta: f32,
    /// Maximum allowed mean absolute per-sample delta.
    pub mean_abs_delta: f32,
}

impl Default for Dx7RenderTolerance {
    fn default() -> Self {
        Self {
            max_abs_delta: 1.25,
            mean_abs_delta: 0.75,
        }
    }
}

/// Measured outcome of comparing the algorithmic and modeled renderers for one
/// fixture against its [`Dx7RenderTolerance`].
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7RenderToleranceReport {
    /// Identifier of the compared fixture.
    pub fixture_id: String,
    /// Number of samples compared across all channels.
    pub frames: usize,
    /// Largest absolute per-sample delta observed.
    pub max_abs_delta: f32,
    /// Mean absolute per-sample delta observed.
    pub mean_abs_delta: f32,
    /// Peak absolute sample of the algorithmic render.
    pub algorithmic_peak: f32,
    /// Peak absolute sample of the compatible (modeled) render.
    pub compatible_peak: f32,
    /// Whether both deltas fall within the fixture tolerance.
    pub passed: bool,
}

/// Accuracy state of the modeled "compatible" DX7 renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dx7RendererAccuracyStatus {
    /// Bit-exact oracle is not yet available; sign-off is withheld.
    IncompleteExactOraclePending,
}

impl Dx7RendererAccuracyStatus {
    /// Returns the stable slug for this accuracy status.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IncompleteExactOraclePending => "incomplete-exact-oracle-pending",
        }
    }
}

/// Sign-off gate tracking which renderer and fixtures the accuracy oracle
/// covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7RendererAccuracyGate {
    /// Renderer backend the gate applies to.
    pub renderer: ComponentBackend,
    /// Current accuracy status of the gated renderer.
    pub status: Dx7RendererAccuracyStatus,
    /// Fixture ids that must pass before the gate can clear.
    pub required_fixture_ids: Vec<String>,
}

impl Dx7RendererAccuracyGate {
    /// Returns whether a bit-exact oracle is available; always `false` while the
    /// oracle is pending.
    pub fn exact_oracle_ready(&self) -> bool {
        false
    }
}

/// Smoke fixture that loads a synthetic bank of one patch per DX7 algorithm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dx7SyntheticBankFixture {
    /// Stable fixture identifier.
    pub id: &'static str,
    /// Sample rate associated with the bank, in hertz.
    pub sample_rate_hz: u32,
    /// Path the user SysEx bank is expected at on disk.
    pub user_sysex_path: &'static str,
    /// One synthetic patch per algorithm.
    pub patches: Vec<Dx7Patch>,
    /// FNV-64 hex hashes paired with `patches` by index.
    pub patch_hashes: Vec<String>,
}

/// Returns the command that regenerates the DX7 render fixtures.
pub fn dx7_fixture_regeneration_command() -> &'static str {
    DX7_FIXTURE_REGENERATE_COMMAND
}

/// Builds the full set of DX7 render fixtures: single-op, two-op modulation,
/// one per algorithm, and an envelope gate/release case.
pub fn dx7_render_fixtures() -> Vec<Dx7RenderFixture> {
    let mut fixtures = vec![
        dx7_render_fixture(
            "dx7-single-op",
            Dx7RenderFixtureKind::SingleOperator,
            single_operator_patch(),
            note_sequence(0, 60, 100, 48),
            64,
        ),
        dx7_render_fixture(
            "dx7-two-op-modulation",
            Dx7RenderFixtureKind::TwoOperatorModulation,
            two_operator_modulation_patch(),
            note_sequence(0, 60, 112, 48),
            64,
        ),
    ];

    for algorithm in 1..=DX7_ALGORITHM_COUNT as u8 {
        fixtures.push(dx7_render_fixture(
            format!("dx7-algorithm-{algorithm:02}"),
            Dx7RenderFixtureKind::Algorithm(algorithm),
            synthetic_dx7_patch(algorithm),
            note_sequence(0, 60 + algorithm % 12, 96, 48),
            64,
        ));
    }

    fixtures.push(dx7_render_fixture(
        "dx7-envelope-gate-release",
        Dx7RenderFixtureKind::Envelope,
        envelope_patch(),
        vec![
            BlockEvent::NoteOn {
                offset: 0,
                channel: 0,
                key: 57,
                velocity: 0.9,
            },
            BlockEvent::NoteOff {
                offset: 40,
                channel: 0,
                key: 57,
                velocity: 0.0,
            },
            BlockEvent::Midi {
                offset: 56,
                bytes: [0xb0, 64, 0],
                len: 3,
            },
        ],
        96,
    ));

    fixtures
}

/// Returns every render-fixture id plus the synthetic bank fixture id.
pub fn dx7_render_fixture_ids() -> Vec<String> {
    let mut ids = dx7_render_fixtures()
        .into_iter()
        .map(|fixture| fixture.id)
        .collect::<Vec<_>>();
    ids.push(dx7_synthetic_bank_fixture().id.to_owned());
    ids
}

/// Builds the synthetic bank fixture: one patch per algorithm at 48 kHz with
/// matching patch hashes.
pub fn dx7_synthetic_bank_fixture() -> Dx7SyntheticBankFixture {
    let patches = (1..=DX7_ALGORITHM_COUNT as u8)
        .map(synthetic_dx7_patch)
        .collect::<Vec<_>>();
    let patch_hashes = patches.iter().map(hash_patch).collect();
    Dx7SyntheticBankFixture {
        id: "dx7-synthetic-bank-load-smoke",
        sample_rate_hz: 48_000,
        user_sysex_path: "$HOME/.local/share/sim/dx7/user-bank.syx",
        patches,
        patch_hashes,
    }
}

/// Renders a fixture with the algorithmic backend and returns per-channel
/// sample buffers.
pub fn render_dx7_fixture(fixture: &Dx7RenderFixture) -> Vec<Vec<f32>> {
    let mut voice = Dx7Voice::new(fixture.patch.clone(), ComponentBackend::Algorithmic);
    render_processor(
        &mut voice,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    )
}

/// Renders a fixture on both the algorithmic and modeled backends and reports
/// their sample deltas against the fixture tolerance.
pub fn dx7_compatible_renderer_tolerance_report(
    fixture: &Dx7RenderFixture,
) -> Dx7RenderToleranceReport {
    let mut algorithmic = Dx7Voice::new(fixture.patch.clone(), ComponentBackend::Algorithmic);
    let mut compatible = Dx7Voice::new(fixture.patch.clone(), ComponentBackend::Modeled);
    let left = render_processor(
        &mut algorithmic,
        &fixture.events,
        fixture.sample_rate_hz,
        fixture.frames,
        fixture.channels,
    );
    let right = render_processor(
        &mut compatible,
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
    Dx7RenderToleranceReport {
        fixture_id: fixture.id.clone(),
        frames,
        max_abs_delta,
        mean_abs_delta,
        algorithmic_peak: peak(&left),
        compatible_peak: peak(&right),
        passed: max_abs_delta <= fixture.tolerance.max_abs_delta
            && mean_abs_delta <= fixture.tolerance.mean_abs_delta,
    }
}

/// Returns the accuracy gate for the modeled renderer, listing every required
/// fixture id.
pub fn dx7_accurate_renderer_gate() -> Dx7RendererAccuracyGate {
    Dx7RendererAccuracyGate {
        renderer: ComponentBackend::Modeled,
        status: Dx7RendererAccuracyStatus::IncompleteExactOraclePending,
        required_fixture_ids: dx7_render_fixture_ids(),
    }
}

/// Renders the full DX7 fixture manifest as TOML, covering every fixture and
/// the synthetic bank.
pub fn dx7_render_fixture_manifest() -> String {
    let mut out = String::new();
    out.push_str("# Generated by `cargo run -p sim-lib-music-synth --bin dx7-fixtures`.\n");
    out.push_str(&format!(
        "regenerate_command = \"{}\"\n",
        toml_escape(DX7_FIXTURE_REGENERATE_COMMAND)
    ));
    out.push_str(&format!(
        "accurate_renderer_status = \"{}\"\n\n",
        dx7_accurate_renderer_gate().status.as_str()
    ));

    for fixture in dx7_render_fixtures() {
        out.push_str("[[fixture]]\n");
        out.push_str(&format!("id = \"{}\"\n", toml_escape(&fixture.id)));
        out.push_str(&format!(
            "kind = \"{}\"\n",
            toml_escape(&fixture.kind.as_str())
        ));
        out.push_str(&format!(
            "sample_rate_hz = {}\nframes = {}\nchannels = {}\n",
            fixture.sample_rate_hz, fixture.frames, fixture.channels
        ));
        out.push_str(&format!(
            "mode = \"{}\"\npatch_hash = \"{}\"\n",
            toml_escape(&fixture.metadata.mode),
            fixture.metadata.patch_hash
        ));
        out.push_str(&format!(
            "midi_sequence = [{}]\n",
            toml_array(&fixture.metadata.midi_sequence)
        ));
        out.push_str(&format!(
            "component_versions = [{}]\n",
            toml_array(&fixture.metadata.component_versions)
        ));
        out.push_str(&format!(
            "compatible_max_abs_delta = {:.3}\ncompatible_mean_abs_delta = {:.3}\n\n",
            fixture.tolerance.max_abs_delta, fixture.tolerance.mean_abs_delta
        ));
    }

    let bank = dx7_synthetic_bank_fixture();
    out.push_str("[[bank]]\n");
    out.push_str(&format!("id = \"{}\"\n", bank.id));
    out.push_str(&format!("sample_rate_hz = {}\n", bank.sample_rate_hz));
    out.push_str(&format!(
        "user_sysex_path = \"{}\"\n",
        toml_escape(bank.user_sysex_path)
    ));
    out.push_str(&format!("patch_count = {}\n", bank.patches.len()));
    out.push_str(&format!(
        "patch_hashes = [{}]\n",
        toml_array(&bank.patch_hashes)
    ));
    out
}

fn dx7_render_fixture(
    id: impl Into<String>,
    kind: Dx7RenderFixtureKind,
    patch: Dx7Patch,
    events: Vec<BlockEvent<'static>>,
    frames: usize,
) -> Dx7RenderFixture {
    let id = id.into();
    let sample_rate_hz = 48_000;
    let channels = 1;
    let tolerance = Dx7RenderTolerance::default();
    let metadata = dx7_fixture_metadata(&patch, &events, sample_rate_hz);
    Dx7RenderFixture {
        id,
        kind,
        patch,
        events,
        sample_rate_hz,
        frames,
        channels,
        tolerance,
        metadata,
    }
}

fn dx7_fixture_metadata(
    patch: &Dx7Patch,
    events: &[BlockEvent<'static>],
    sample_rate_hz: u32,
) -> Dx7RenderFixtureMetadata {
    Dx7RenderFixtureMetadata {
        sample_rate_hz,
        mode: ComponentBackend::Algorithmic.as_str().to_owned(),
        patch_hash: hash_patch(patch),
        midi_sequence: events.iter().copied().map(event_label).collect(),
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

fn single_operator_patch() -> Dx7Patch {
    let mut patch = synthetic_dx7_patch(32);
    patch.name = "dx7-single-op".to_owned();
    for (index, operator) in patch.operators.iter_mut().enumerate() {
        operator.output_level = if index == 0 { 82 } else { 0 };
        operator.frequency_coarse = 1;
        operator.frequency_fine = 0;
    }
    patch
}

fn two_operator_modulation_patch() -> Dx7Patch {
    let mut patch = synthetic_dx7_patch(8);
    patch.name = "dx7-two-op-modulation".to_owned();
    for operator in &mut patch.operators {
        operator.output_level = 0;
        operator.frequency_coarse = 1;
        operator.frequency_fine = 0;
    }
    patch.operators[0].output_level = 78;
    patch.operators[5].output_level = 86;
    patch.feedback = 3;
    patch
}

fn envelope_patch() -> Dx7Patch {
    let mut patch = synthetic_dx7_patch(16);
    patch.name = "dx7-envelope-gate-release".to_owned();
    for (index, operator) in patch.operators.iter_mut().enumerate() {
        operator.rates = [99, 75, 42, 55];
        operator.levels = [99, 88, 52, 0];
        operator.output_level = 50 + index as u8 * 5;
    }
    patch
}

fn synthetic_dx7_patch(algorithm: u8) -> Dx7Patch {
    let operators = (0..DX7_OPERATOR_COUNT)
        .map(|index| Dx7PatchOperator {
            rates: [99, 84 - index as u8, 70 - index as u8, 50 + index as u8],
            levels: [99, 82 - index as u8, 58 + index as u8, 0],
            breakpoint: 36 + index as u8,
            left_depth: index as u8,
            right_depth: 2 * index as u8,
            left_curve: index as u8 % 4,
            right_curve: (index as u8 + 1) % 4,
            rate_scale: index as u8 % 7,
            amp_mod_sens: index as u8 % 4,
            key_velocity_sens: (index as u8 + algorithm) % 8,
            output_level: 45 + index as u8 * 7,
            oscillator_mode: 0,
            frequency_coarse: 1 + index as u8,
            frequency_fine: (algorithm + index as u8 * 3) % 100,
            detune: 7 + index as u8 % 3,
        })
        .collect::<Vec<_>>();
    Dx7Patch {
        name: format!("dx7-synthetic-{algorithm:02}"),
        operators,
        algorithm,
        feedback: algorithm % 8,
        lfo: Dx7Lfo {
            speed: 35 + algorithm % 20,
            pitch_mod_depth: algorithm % 16,
            amp_mod_depth: algorithm % 8,
            pitch_mod_sens: algorithm % 4,
            ..Dx7Lfo::default()
        },
        raw: Dx7RawPatch {
            edit_buffer: synthetic_raw_bytes(algorithm, 155),
            packed_voice: synthetic_raw_bytes(algorithm.wrapping_add(17), 128),
        },
        ..Dx7Patch::default()
    }
}

fn synthetic_raw_bytes(seed: u8, len: usize) -> Vec<u8> {
    (0..len)
        .map(|index| seed.wrapping_add(index as u8).wrapping_mul(3) & 0x7f)
        .collect()
}

fn component_versions() -> Vec<String> {
    [
        "sim-lib-music-synth",
        "Dx7Voice",
        "Dx7FmOperator",
        "Dx7ModeledOperator",
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

fn hash_patch(patch: &Dx7Patch) -> String {
    let mut hash = Fnv64::new();
    hash.bytes(patch.name.as_bytes());
    hash.u8(patch.algorithm);
    hash.u8(patch.feedback);
    hash.u8(patch.transpose);
    hash.u8(u8::from(patch.oscillator_sync));
    for operator in &patch.operators {
        hash.bytes(&operator.rates);
        hash.bytes(&operator.levels);
        hash.u8(operator.breakpoint);
        hash.u8(operator.left_depth);
        hash.u8(operator.right_depth);
        hash.u8(operator.left_curve);
        hash.u8(operator.right_curve);
        hash.u8(operator.rate_scale);
        hash.u8(operator.amp_mod_sens);
        hash.u8(operator.key_velocity_sens);
        hash.u8(operator.output_level);
        hash.u8(operator.oscillator_mode);
        hash.u8(operator.frequency_coarse);
        hash.u8(operator.frequency_fine);
        hash.u8(operator.detune);
    }
    hash.bytes(&patch.pitch_envelope.rates);
    hash.bytes(&patch.pitch_envelope.levels);
    hash.u8(patch.lfo.speed);
    hash.u8(patch.lfo.delay);
    hash.u8(patch.lfo.pitch_mod_depth);
    hash.u8(patch.lfo.amp_mod_depth);
    hash.u8(u8::from(patch.lfo.sync));
    hash.u8(patch.lfo.waveform);
    hash.u8(patch.lfo.pitch_mod_sens);
    hash.bytes(&patch.raw.edit_buffer);
    hash.bytes(&patch.raw.packed_voice);
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
