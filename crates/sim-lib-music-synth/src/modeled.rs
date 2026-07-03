use sim_kernel::Symbol;
use sim_lib_audio_graph_core::{BlockEvent, ProcessBlock};

use crate::{
    ComponentBackend, ComponentBackendSurface, ComponentInspection, ComponentParamDescriptor,
    ComponentPortDescriptor, ComponentPrepareConfig, ComponentTick, ComponentTickResult,
    ComponentTraceFrame, ComponentTraceValue, DiscreteComponent, Dx7EgsEnvelope, Dx7EgsPitch,
    Dx7FloatingDac, Dx7FmOperatorSettings, Dx7OperatorInput, Dx7OpsDatapath, Dx7OpsInput, QLevel,
    QPhase, dx7_operator_params, dx7_operator_ports,
};

/// Names of the integer trace fixtures emitted by the modeled operator.
pub const DX7_MODELED_TRACE_FIXTURES: [&str; 3] = [
    "dx7-modeled-single-op-i32",
    "dx7-modeled-divergence-report",
    "dx7-modeled-integer-trace",
];

/// One sample produced by the modeled operator, with its integer trace frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7ModeledOperatorOutput {
    /// Output sample as a fixed-point level.
    pub sample: QLevel,
    /// Per-sample trace of the modeled datapath words.
    pub trace: ComponentTraceFrame,
}

/// Summary of integer-sample divergence between the algorithmic and modeled
/// renderers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx7ModeledDivergenceReport {
    /// Number of samples compared.
    pub frames: usize,
    /// Largest absolute per-sample integer delta.
    pub max_abs_delta: i32,
    /// Sum of absolute per-sample integer deltas.
    pub sum_abs_delta: i64,
}

/// Cycle-accurate modeled DX7 operator chaining the EGS, OPS, and floating DAC
/// stages and emitting integer traces for divergence checking.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7ModeledOperator {
    settings: Dx7FmOperatorSettings,
    sample_rate_hz: u32,
    phase: QPhase,
    envelope: Dx7EgsEnvelope,
    pitch: Dx7EgsPitch,
    ops: Dx7OpsDatapath,
    dac: Dx7FloatingDac,
    clock: u64,
    gate: bool,
    key: u8,
    velocity: f32,
    last_trace: Option<ComponentTraceFrame>,
}

impl Dx7ModeledOperator {
    /// Creates a modeled operator from FM settings at the default 48 kHz rate.
    pub fn new(settings: Dx7FmOperatorSettings) -> Self {
        let sample_rate_hz = 48_000;
        Self {
            sample_rate_hz,
            phase: QPhase::ZERO,
            envelope: Dx7EgsEnvelope::new(settings.envelope),
            pitch: Dx7EgsPitch::new(sample_rate_hz),
            ops: Dx7OpsDatapath::new(),
            dac: Dx7FloatingDac::default(),
            clock: 0,
            gate: false,
            key: settings.base_key,
            velocity: settings.base_velocity,
            last_trace: None,
            settings,
        }
    }

    /// Returns the operator's FM settings.
    pub fn settings(&self) -> &Dx7FmOperatorSettings {
        &self.settings
    }

    /// Returns the current oscillator phase.
    pub fn phase(&self) -> QPhase {
        self.phase
    }

    /// Returns the OPS datapath feedback delay line.
    pub fn ops_feedback(&self) -> [i32; 2] {
        self.ops.feedback()
    }

    /// Updates the operator's sample rate (clamped to >= 1) and its pitch stage.
    pub fn set_sample_rate(&mut self, sample_rate_hz: u32) {
        self.sample_rate_hz = sample_rate_hz.max(1);
        self.pitch.set_sample_rate(self.sample_rate_hz);
    }

    /// Renders one operator sample from the given input and returns it with its
    /// integer trace frame.
    pub fn next_sample(&mut self, input: Dx7OperatorInput) -> Dx7ModeledOperatorOutput {
        self.key = input.key;
        self.velocity = input.velocity.clamp(0.0, 1.0);
        self.gate = input.gate;

        let rate_boost = self.settings.scaling.rate_boost(self.key);
        let envelope = self.envelope.next_level(self.gate, rate_boost);
        let scaled_envelope = self.scaled_envelope_word(envelope);
        let delta =
            self.pitch
                .phase_delta(self.settings.pitch, self.key, input.pitch_mod_semitones);
        let pitch_word = self.pitch.pitch_word(
            self.settings.pitch,
            self.key,
            (input.pitch_mod_semitones * 128.0).round() as i16,
        );
        let modulation = input.modulation.rounded_shift_right(17);
        let ops = self.ops.next(Dx7OpsInput {
            phase: self.phase,
            modulation,
            envelope: scaled_envelope,
            feedback: self.settings.feedback,
        });
        let dac_sample = self.dac.next_sample(ops.raw);
        let sample = QLevel::from_f32(dac_sample);
        let trace = self.trace_frame(ModeledTraceInput {
            delta,
            pitch_word,
            modulation,
            envelope,
            scaled_envelope,
            ops,
            dac_sample,
            sample,
        });
        self.phase.advance_wrapping(delta);
        self.clock = self.clock.saturating_add(1);
        self.last_trace = Some(trace.clone());
        Dx7ModeledOperatorOutput { sample, trace }
    }

    fn scaled_envelope_word(&self, envelope: u16) -> u16 {
        let output = u32::from(self.settings.output_level.min(99));
        let keyboard = (self.settings.scaling.level_gain(self.key) * 1024.0)
            .round()
            .clamp(0.0, 2048.0) as u32;
        let velocity = (self.settings.velocity.gain(self.velocity) * 1024.0)
            .round()
            .clamp(0.0, 1024.0) as u32;
        let scaled =
            u64::from(envelope) * u64::from(output) * u64::from(keyboard) * u64::from(velocity);
        (scaled / (99 * 1024 * 1024)).min(u64::from(u16::MAX)) as u16
    }

    fn trace_frame(&self, input: ModeledTraceInput) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            dx7_modeled_operator_component_id(),
            ComponentBackend::Modeled,
            self.clock,
        )
        .with_integer(trace_key("phase-raw"), u32_to_i64(self.phase.raw()))
        .with_integer(trace_key("phase-delta-raw"), u32_to_i64(input.delta.raw()))
        .with_integer(trace_key("pitch-word"), i64::from(input.pitch_word))
        .with_integer(trace_key("modulation-i32"), i64::from(input.modulation))
        .with_integer(trace_key("egs-level-word"), i64::from(input.envelope))
        .with_integer(
            trace_key("scaled-egs-level-word"),
            i64::from(input.scaled_envelope),
        )
        .with_integer(
            trace_key("ops-phase-index"),
            i64::from(input.ops.phase_index),
        )
        .with_integer(trace_key("ops-log-sine"), i64::from(input.ops.log_sine))
        .with_integer(trace_key("ops-exp-level"), i64::from(input.ops.exp_level))
        .with_integer(
            trace_key("ops-feedback-average"),
            i64::from(input.ops.feedback_average),
        )
        .with_integer(trace_key("ops-cascade-raw"), i64::from(input.ops.cascade))
        .with_integer(trace_key("ops-output-raw"), i64::from(input.ops.raw))
        .with_integer(trace_key("output-raw"), i64::from(input.sample.raw()))
        .with_state(
            trace_key("dac-sample"),
            ComponentTraceValue::Float(input.dac_sample as f64),
        )
    }

    fn handle_event(&mut self, event: BlockEvent<'_>) {
        match event {
            BlockEvent::NoteOn { key, velocity, .. } if velocity > 0.0 => {
                self.key = key;
                self.velocity = velocity.clamp(0.0, 1.0);
                self.gate = true;
            }
            BlockEvent::NoteOn { key, .. } | BlockEvent::NoteOff { key, .. } if key == self.key => {
                self.gate = false;
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ModeledTraceInput {
    delta: QPhase,
    pitch_word: i32,
    modulation: i32,
    envelope: u16,
    scaled_envelope: u16,
    ops: crate::Dx7OpsOutput,
    dac_sample: f32,
    sample: QLevel,
}

impl Default for Dx7ModeledOperator {
    fn default() -> Self {
        Self::new(Dx7FmOperatorSettings::default())
    }
}

impl DiscreteComponent for Dx7ModeledOperator {
    fn component_id(&self) -> Symbol {
        dx7_modeled_operator_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Modeled
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        dx7_operator_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        dx7_operator_params()
    }

    fn reset(&mut self) {
        self.phase = QPhase::ZERO;
        self.envelope.reset();
        self.ops.reset();
        self.dac.reset();
        self.clock = 0;
        self.gate = false;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            for event in block.in_events {
                if event_offset(*event) == frame as u32 {
                    self.handle_event(*event);
                }
            }
            let modulation = block
                .in_audio
                .first()
                .and_then(|channel| channel.get(frame))
                .copied()
                .unwrap_or(0.0);
            let output = self.next_sample(Dx7OperatorInput {
                key: self.key,
                velocity: self.velocity,
                gate: self.gate,
                modulation: QLevel::from_f32(modulation),
                pitch_mod_semitones: 0.0,
            });
            for channel in &mut *block.out_audio {
                channel[frame] = output.sample.to_f32();
            }
        }
    }

    fn tick(&mut self, tick: ComponentTick) -> ComponentTickResult {
        let output = self.next_sample(Dx7OperatorInput {
            key: self.key,
            velocity: self.velocity,
            gate: tick.gate,
            modulation: QLevel::from_f32(tick.input),
            pitch_mod_semitones: 0.0,
        });
        ComponentTickResult {
            output: output.sample.to_f32(),
            trace: Some(output.trace),
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            dx7_modeled_operator_component_id(),
            ComponentBackend::Modeled,
            self.gate,
        )
        .with_field(trace_key("key"), self.key.to_string())
        .with_field(trace_key("velocity"), self.velocity.to_string())
        .with_field(trace_key("clock"), self.clock.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the qualified component id of the modeled DX7 operator.
pub fn dx7_modeled_operator_component_id() -> Symbol {
    Symbol::qualified("audio-synth/dx7-operator", "modeled")
}

/// Returns the port/param surfaces for both DX7 operator backends.
pub fn dx7_operator_backend_surfaces() -> [ComponentBackendSurface; 2] {
    [
        dx7_operator_backend_surface(ComponentBackend::Algorithmic),
        dx7_operator_backend_surface(ComponentBackend::Modeled),
    ]
}

/// Returns the names of the modeled integer trace fixtures.
pub fn dx7_modeled_trace_fixture_names() -> [&'static str; 3] {
    DX7_MODELED_TRACE_FIXTURES
}

/// Computes the integer-sample divergence between algorithmic and modeled
/// renders over their overlapping frames.
pub fn dx7_modeled_divergence_report(
    algorithmic: &[i32],
    modeled: &[i32],
) -> Dx7ModeledDivergenceReport {
    let frames = algorithmic.len().min(modeled.len());
    let mut max_abs_delta = 0_i32;
    let mut sum_abs_delta = 0_i64;
    for index in 0..frames {
        let delta = i64::from(algorithmic[index]) - i64::from(modeled[index]);
        let abs = delta.abs().min(i64::from(i32::MAX));
        max_abs_delta = max_abs_delta.max(abs as i32);
        sum_abs_delta = sum_abs_delta.saturating_add(abs);
    }
    Dx7ModeledDivergenceReport {
        frames,
        max_abs_delta,
        sum_abs_delta,
    }
}

fn dx7_operator_backend_surface(backend: ComponentBackend) -> ComponentBackendSurface {
    ComponentBackendSurface::new(backend, dx7_operator_ports(), dx7_operator_params())
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/dx7-modeled-trace", name)
}

fn event_offset(event: BlockEvent<'_>) -> u32 {
    match event {
        BlockEvent::Midi { offset, .. }
        | BlockEvent::MidiLong { offset, .. }
        | BlockEvent::ParamSet { offset, .. }
        | BlockEvent::NoteOn { offset, .. }
        | BlockEvent::NoteOff { offset, .. } => offset,
    }
}

fn u32_to_i64(value: u32) -> i64 {
    i64::from(value)
}
