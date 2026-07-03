use sim_kernel::Symbol;
use sim_lib_audio_graph_core::{BlockEvent, ProcessBlock};

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTick, ComponentTickResult, ComponentTraceFrame,
    ComponentTraceValue, DiscreteComponent, Dx7AlgorithmicLfo, Dx7AlgorithmicLfoSettings,
    Dx7EnvelopeGenerator, Dx7EnvelopeSettings, Dx7KeyboardScaling, Dx7Patch, Dx7PatchOperator,
    Dx7PitchEnvelope, Dx7PitchEnvelopeSettings, Dx7PitchSettings, Dx7VelocitySensitivity,
    GeneratedLut, QLevel, QPhase, dx7_patch_component_kind,
};

/// Names of the golden trace fixtures that exercise the algorithmic operator:
/// single operator, two-operator modulation, and feedback.
pub const DX7_OPERATOR_TRACE_FIXTURES: [&str; 3] = [
    "dx7-operator-single-op-i32",
    "dx7-operator-two-op-modulation-i32",
    "dx7-operator-feedback-i32",
];

/// Per-sample input to a DX7 FM operator: the played note, gate, and incoming
/// phase/pitch modulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dx7OperatorInput {
    /// MIDI key number driving operator pitch and scaling.
    pub key: u8,
    /// Note velocity in 0.0..=1.0 used for velocity sensitivity.
    pub velocity: f32,
    /// Whether the note gate is open, driving the envelope.
    pub gate: bool,
    /// Phase modulation from modulating operators (FM input).
    pub modulation: QLevel,
    /// Additional pitch offset in semitones (pitch bend, mod wheel, etc.).
    pub pitch_mod_semitones: f32,
}

impl Default for Dx7OperatorInput {
    fn default() -> Self {
        Self {
            key: 60,
            velocity: 1.0,
            gate: false,
            modulation: QLevel::ZERO,
            pitch_mod_semitones: 0.0,
        }
    }
}

/// The result of advancing a DX7 FM operator one sample: the output level and
/// a trace frame of its internal state.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7OperatorOutput {
    /// The operator's output level for this sample.
    pub sample: QLevel,
    /// Trace frame capturing phase, modulation, envelope, and output state.
    pub trace: ComponentTraceFrame,
}

/// Complete configuration of a single DX7 FM operator, gathering its pitch,
/// envelopes, LFO, scaling, velocity, and level/feedback parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7FmOperatorSettings {
    /// Operator pitch tuning (ratio/fixed, coarse, fine, detune).
    pub pitch: Dx7PitchSettings,
    /// Amplitude envelope shape.
    pub envelope: Dx7EnvelopeSettings,
    /// Pitch envelope shape applied to the operator frequency.
    pub pitch_envelope: Dx7PitchEnvelopeSettings,
    /// LFO settings for pitch and amplitude modulation.
    pub lfo: Dx7AlgorithmicLfoSettings,
    /// Keyboard level and rate scaling.
    pub scaling: Dx7KeyboardScaling,
    /// Velocity-to-level sensitivity.
    pub velocity: Dx7VelocitySensitivity,
    /// Operator output level (0..=99).
    pub output_level: u8,
    /// Amplitude modulation sensitivity to the LFO (0..=3).
    pub amp_mod_sens: u8,
    /// Self-feedback amount (0..=7), nonzero only on feedback operators.
    pub feedback: u8,
    /// Reference MIDI key used as the operator's starting note.
    pub base_key: u8,
    /// Reference velocity used as the operator's starting velocity.
    pub base_velocity: f32,
    /// Length of the generated sine lookup table.
    pub sine_lut_len: usize,
}

impl Dx7FmOperatorSettings {
    /// Builds operator settings from a patch operator and its parent patch,
    /// clamping levels and pulling in the shared LFO and pitch envelope.
    pub fn from_patch_operator(operator: &Dx7PatchOperator, patch: &Dx7Patch) -> Self {
        Self {
            pitch: Dx7PitchSettings::from_patch_operator(operator),
            envelope: Dx7EnvelopeSettings::new(operator.rates, operator.levels),
            pitch_envelope: Dx7PitchEnvelopeSettings::from_patch_envelope(
                &patch.pitch_envelope,
                2.0,
            ),
            lfo: Dx7AlgorithmicLfoSettings::from_patch_lfo(&patch.lfo),
            scaling: Dx7KeyboardScaling::from_patch_operator(operator),
            velocity: Dx7VelocitySensitivity::new(operator.key_velocity_sens),
            output_level: operator.output_level.min(99),
            amp_mod_sens: operator.amp_mod_sens.min(3),
            feedback: patch.feedback.min(7),
            base_key: 60,
            base_velocity: 1.0,
            sine_lut_len: 1024,
        }
    }
}

impl Default for Dx7FmOperatorSettings {
    fn default() -> Self {
        Self {
            pitch: Dx7PitchSettings::default(),
            envelope: Dx7EnvelopeSettings::constant(99),
            pitch_envelope: Dx7PitchEnvelopeSettings::default(),
            lfo: Dx7AlgorithmicLfoSettings::default(),
            scaling: Dx7KeyboardScaling::default(),
            velocity: Dx7VelocitySensitivity::default(),
            output_level: 99,
            amp_mod_sens: 0,
            feedback: 0,
            base_key: 60,
            base_velocity: 1.0,
            sine_lut_len: 1024,
        }
    }
}

/// A single algorithmic DX7 FM operator: a sine oscillator with phase
/// modulation, amplitude and pitch envelopes, an LFO, and self-feedback.
#[derive(Clone, Debug, PartialEq)]
pub struct Dx7FmOperator {
    settings: Dx7FmOperatorSettings,
    sample_rate_hz: f32,
    phase: QPhase,
    envelope: Dx7EnvelopeGenerator,
    pitch_envelope: Dx7PitchEnvelope,
    lfo: Dx7AlgorithmicLfo,
    sine: GeneratedLut,
    feedback_delay: QLevel,
    clock: u64,
    gate: bool,
    key: u8,
    velocity: f32,
    last_trace: Option<ComponentTraceFrame>,
}

impl Dx7FmOperator {
    /// Creates an operator from `settings`, building its envelopes, LFO, and
    /// sine table at a default 48 kHz sample rate.
    pub fn new(settings: Dx7FmOperatorSettings) -> Self {
        let mut operator = Self {
            sample_rate_hz: 48_000.0,
            phase: QPhase::ZERO,
            envelope: Dx7EnvelopeGenerator::new(settings.envelope),
            pitch_envelope: Dx7PitchEnvelope::new(settings.pitch_envelope),
            lfo: Dx7AlgorithmicLfo::new(settings.lfo),
            sine: GeneratedLut::sine(settings.sine_lut_len),
            feedback_delay: QLevel::ZERO,
            clock: 0,
            gate: false,
            key: settings.base_key,
            velocity: settings.base_velocity,
            last_trace: None,
            settings,
        };
        operator.set_sample_rate(48_000.0);
        operator
    }

    /// Returns the operator's configuration.
    pub fn settings(&self) -> &Dx7FmOperatorSettings {
        &self.settings
    }

    /// Returns the current oscillator phase.
    pub fn phase(&self) -> QPhase {
        self.phase
    }

    /// Returns the last output sample held in the feedback delay.
    pub fn feedback_delay(&self) -> QLevel {
        self.feedback_delay
    }

    /// Sets the playback sample rate in Hz (clamped to at least 1.0) and
    /// propagates it to the envelopes and LFO.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
        self.envelope.set_sample_rate(self.sample_rate_hz);
        self.pitch_envelope.set_sample_rate(self.sample_rate_hz);
        self.lfo.set_sample_rate(self.sample_rate_hz);
    }

    /// Advances the operator one sample for `input`, returning the modulated
    /// sine output scaled by the envelope, level, scaling, velocity, and LFO
    /// amplitude, together with a trace of the computation.
    pub fn next_sample(&mut self, input: Dx7OperatorInput) -> Dx7OperatorOutput {
        self.key = input.key;
        self.velocity = input.velocity.clamp(0.0, 1.0);
        self.gate = input.gate;

        let rate_boost = self.settings.scaling.rate_boost(input.key);
        let envelope = self.envelope.next_level(input.gate, rate_boost);
        let pitch_env = self.pitch_envelope.next_semitones(input.gate);
        let lfo = self.lfo.next_frame();
        let frequency = self.settings.pitch.frequency_hz(
            input.key,
            input.pitch_mod_semitones + pitch_env + lfo.pitch_semitones,
        );
        let delta = QPhase::from_turns(f64::from(frequency / self.sample_rate_hz));
        let feedback = self.feedback_delay.to_f32() * f32::from(self.settings.feedback) / 7.0;
        let modulation_turns = input.modulation.to_f32() * 0.125 + feedback * 0.125;
        let sample_phase = self
            .phase
            .wrapping_add(QPhase::from_turns(f64::from(modulation_turns)));
        let sine = QLevel::from_f32(self.sine.sample_phase(sample_phase));
        let gain = self.output_gain(input.key, input.velocity, lfo.amp);
        let level = envelope.saturating_mul(QLevel::from_f32(gain));
        let sample = sine.saturating_mul(level);

        let trace = self.trace_frame(delta, input.modulation, envelope, lfo.amp, sample);
        self.feedback_delay = sample;
        self.phase.advance_wrapping(delta);
        self.clock = self.clock.saturating_add(1);
        self.last_trace = Some(trace.clone());
        Dx7OperatorOutput { sample, trace }
    }

    fn output_gain(&self, key: u8, velocity: f32, lfo_amp: QLevel) -> f32 {
        let output = f32::from(self.settings.output_level) / 99.0;
        let keyboard = self.settings.scaling.level_gain(key);
        let velocity = self.settings.velocity.gain(velocity);
        let amp_mod = 1.0 - lfo_amp.to_f32().abs() * f32::from(self.settings.amp_mod_sens) / 3.0;
        (output * keyboard * velocity * amp_mod).clamp(0.0, 1.999)
    }

    fn trace_frame(
        &self,
        delta: QPhase,
        modulation: QLevel,
        envelope: QLevel,
        lfo_amp: QLevel,
        sample: QLevel,
    ) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            dx7_operator_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_integer(trace_key("phase-raw"), u32_to_i64(self.phase.raw()))
        .with_integer(trace_key("phase-delta-raw"), u32_to_i64(delta.raw()))
        .with_integer(trace_key("modulation-raw"), i64::from(modulation.raw()))
        .with_integer(
            trace_key("feedback-delay-raw"),
            i64::from(self.feedback_delay.raw()),
        )
        .with_integer(trace_key("envelope-raw"), i64::from(envelope.raw()))
        .with_integer(trace_key("lfo-amp-raw"), i64::from(lfo_amp.raw()))
        .with_integer(trace_key("output-raw"), i64::from(sample.raw()))
        .with_state(
            trace_key("frequency-hz"),
            ComponentTraceValue::Float(self.frequency_for_trace() as f64),
        )
    }

    fn frequency_for_trace(&self) -> f32 {
        self.settings.pitch.frequency_hz(self.key, 0.0)
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

impl Default for Dx7FmOperator {
    /// Returns an operator built from the default settings.
    fn default() -> Self {
        Self::new(Dx7FmOperatorSettings::default())
    }
}

impl DiscreteComponent for Dx7FmOperator {
    fn component_id(&self) -> Symbol {
        dx7_operator_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
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
        self.pitch_envelope.reset();
        self.lfo.reset();
        self.feedback_delay = QLevel::ZERO;
        self.clock = 0;
        self.gate = false;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
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
            dx7_operator_component_id(),
            ComponentBackend::Algorithmic,
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

/// Returns the component-kind symbol identifying the DX7 operator component.
pub fn dx7_operator_component_id() -> Symbol {
    dx7_patch_component_kind()
}

/// Returns the port descriptors for a standalone DX7 operator: gate, optional
/// modulation and pitch-mod inputs, an audio output, and an optional trace
/// output.
pub fn dx7_operator_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "gate-in"),
            ComponentPortMedia::Gate,
            ComponentPortDirection::Input,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "modulation-in"),
            ComponentPortMedia::AudioRate,
            ComponentPortDirection::Input,
            1,
        )
        .optional(),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "pitch-mod-in"),
            ComponentPortMedia::ControlRate,
            ComponentPortDirection::Input,
            1,
        )
        .optional(),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "audio-out"),
            ComponentPortMedia::AudioRate,
            ComponentPortDirection::Output,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "trace-out"),
            ComponentPortMedia::Trace,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
    ]
}

/// Returns the parameter descriptors exposed by a DX7 operator: frequency
/// mode, output level, feedback, detune, and velocity sensitivity.
pub fn dx7_operator_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "frequency-mode"),
            "Frequency mode",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                Symbol::qualified("audio-synth/dx7-frequency-mode", "ratio"),
                Symbol::qualified("audio-synth/dx7-frequency-mode", "fixed"),
            ],
            0,
        ),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "output-level"),
            "Output level",
            ComponentParamUnit::RawInteger,
        )
        .with_range(ComponentParamRange::new(0.0, 99.0, 99.0))
        .with_raw_default(99),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "feedback"),
            "Feedback",
            ComponentParamUnit::RawInteger,
        )
        .with_range(ComponentParamRange::new(0.0, 7.0, 0.0))
        .with_raw_default(0),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "detune"),
            "Detune",
            ComponentParamUnit::Semitones,
        )
        .with_range(ComponentParamRange::new(-7.0, 7.0, 0.0)),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "velocity-sensitivity"),
            "Velocity sensitivity",
            ComponentParamUnit::RawInteger,
        )
        .with_range(ComponentParamRange::new(0.0, 7.0, 0.0))
        .with_raw_default(0),
    ]
}

/// Returns the names of the operator golden trace fixtures
/// ([`DX7_OPERATOR_TRACE_FIXTURES`]).
pub fn dx7_operator_trace_fixture_names() -> [&'static str; 3] {
    DX7_OPERATOR_TRACE_FIXTURES
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/dx7-operator-trace", name)
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
