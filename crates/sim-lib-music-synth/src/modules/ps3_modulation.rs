//! PS-3300-style modulation sources and signal conditioning.
//!
//! Provides the control-rail building blocks of the PS-3300: a low-frequency
//! modulation generator, a sample-and-hold, and an external-signal processor
//! that gains, biases, and envelope-follows an outside input into control
//! voltages. Each is a [`DiscreteComponent`] and feeds the tone and cell stages.

use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Fixture names for the modulation conformance scenarios (generator shapes,
/// sample-and-hold edge capture, external-processor tracking).
pub const PS3300_MODULATION_FIXTURE_NAMES: [&str; 3] = [
    "ps3300-ps3-modulation-generator-shapes",
    "ps3300-ps3-sample-hold-edge-capture",
    "ps3300-ps3-external-processor-tracking",
];

/// Waveform shape of the modulation generator's low-frequency oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300ModulationWaveform {
    /// Pure sine wave.
    Sine,
    /// Symmetric triangle wave.
    Triangle,
    /// Rising sawtooth ramp.
    Saw,
    /// Square wave (50% duty cycle).
    Square,
}

impl Ps3300ModulationWaveform {
    /// Returns the stable lowercase identifier for this waveform.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Triangle => "triangle",
            Self::Saw => "saw",
            Self::Square => "square",
        }
    }

    /// Returns the qualified symbol naming this waveform as a parameter value.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-mg-waveform", self.as_str())
    }
}

/// Configuration for a [`Ps3300ModulationGenerator`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ModulationGeneratorSettings {
    /// Oscillator waveform shape.
    pub waveform: Ps3300ModulationWaveform,
    /// Base modulation rate, in Hz.
    pub rate_hz: f32,
    /// Output depth (amplitude scale), in 0.0..=1.0.
    pub depth: f32,
    /// DC offset added before clamping, in -1.0..=1.0.
    pub offset: f32,
    /// Octaves of rate change per volt of rate CV.
    pub rate_cv_depth_octaves: f32,
}

impl Default for Ps3300ModulationGeneratorSettings {
    fn default() -> Self {
        Self {
            waveform: Ps3300ModulationWaveform::Sine,
            rate_hz: 5.0,
            depth: 1.0,
            offset: 0.0,
            rate_cv_depth_octaves: 1.0,
        }
    }
}

/// One rendered sample of the modulation generator in both output ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ModulationFrame {
    /// Bipolar output in -1.0..=1.0.
    pub bipolar: f32,
    /// Unipolar output in 0.0..=1.0, derived from `bipolar`.
    pub unipolar: f32,
    /// Oscillator phase at this sample, in 0.0..1.0.
    pub phase: f32,
}

/// Low-frequency modulation generator with rate CV and bipolar/unipolar rails.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300ModulationGenerator {
    settings: Ps3300ModulationGeneratorSettings,
    sample_rate_hz: f32,
    phase: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300ModulationGenerator {
    /// Builds a modulation generator from the given (sanitized) settings at a
    /// default 48 kHz sample rate.
    pub fn new(settings: Ps3300ModulationGeneratorSettings) -> Self {
        Self {
            settings: sanitize_mg(settings),
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the working sample rate in Hz (floored at 1.0).
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Advances the oscillator one sample under the given rate CV, returning the
    /// bipolar and unipolar outputs.
    pub fn next_sample(&mut self, rate_cv_v: f32) -> Ps3300ModulationFrame {
        let rate = self.effective_rate_hz(rate_cv_v);
        let wave = wave_sample(self.settings.waveform, self.phase);
        let bipolar = (wave * self.settings.depth + self.settings.offset).clamp(-1.0, 1.0);
        let frame = Ps3300ModulationFrame {
            bipolar,
            unipolar: ((bipolar + 1.0) * 0.5).clamp(0.0, 1.0),
            phase: self.phase,
        };
        self.phase = (self.phase + rate / self.sample_rate_hz).fract();
        self.last_trace = Some(self.trace_frame(rate, frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn effective_rate_hz(&self, rate_cv_v: f32) -> f32 {
        let octaves = rate_cv_v * self.settings.rate_cv_depth_octaves;
        (self.settings.rate_hz * 2.0_f32.powf(octaves)).clamp(0.01, self.sample_rate_hz * 0.5)
    }

    fn trace_frame(&self, rate_hz: f32, frame: Ps3300ModulationFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_modulation_generator_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("waveform"),
            ComponentTraceValue::Text(self.settings.waveform.as_str().to_owned()),
        )
        .with_state(
            trace_key("rate-hz"),
            ComponentTraceValue::Float(f64::from(rate_hz)),
        )
        .with_output(
            trace_key("bipolar"),
            ComponentTraceValue::Float(f64::from(frame.bipolar)),
        )
    }
}

impl Default for Ps3300ModulationGenerator {
    fn default() -> Self {
        Self::new(Ps3300ModulationGeneratorSettings::default())
    }
}

impl DiscreteComponent for Ps3300ModulationGenerator {
    fn component_id(&self) -> Symbol {
        ps3_modulation_generator_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_modulation_generator_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_modulation_generator_params()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_sample(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &[output.bipolar, output.unipolar]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_modulation_generator_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("waveform"),
            self.settings.waveform.as_str().to_owned(),
        )
        .with_field(inspect_key("phase"), self.phase.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for a [`Ps3300SampleHold`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300SampleHoldSettings {
    /// Value held before the first capture, in -10.0..=10.0.
    pub initial_value: f32,
    /// Trigger voltage at or above which a rising edge captures, in 0.0..=10.0.
    pub trigger_threshold_v: f32,
}

impl Default for Ps3300SampleHoldSettings {
    fn default() -> Self {
        Self {
            initial_value: 0.0,
            trigger_threshold_v: 0.5,
        }
    }
}

/// One rendered sample of a sample-and-hold stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300SampleHoldFrame {
    /// Currently held value.
    pub held: f32,
    /// True on the sample where a new value was captured.
    pub captured: bool,
}

/// Sample-and-hold that latches its input on a rising trigger edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300SampleHold {
    settings: Ps3300SampleHoldSettings,
    held: f32,
    last_trigger_high: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300SampleHold {
    /// Builds a sample-and-hold from the given (sanitized) settings, seeded with
    /// the initial held value.
    pub fn new(settings: Ps3300SampleHoldSettings) -> Self {
        let settings = sanitize_sample_hold(settings);
        Self {
            settings,
            held: settings.initial_value,
            last_trigger_high: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Advances one sample: captures `signal` when `trigger_v` crosses the
    /// threshold upward, otherwise holds the previous value.
    pub fn next_sample(&mut self, signal: f32, trigger_v: f32) -> Ps3300SampleHoldFrame {
        let trigger_high = trigger_v >= self.settings.trigger_threshold_v;
        let captured = trigger_high && !self.last_trigger_high;
        if captured {
            self.held = signal.clamp(-10.0, 10.0);
        }
        self.last_trigger_high = trigger_high;
        let frame = Ps3300SampleHoldFrame {
            held: self.held,
            captured,
        };
        self.last_trace = Some(self.sample_hold_trace(frame, trigger_high));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn sample_hold_trace(
        &self,
        frame: Ps3300SampleHoldFrame,
        trigger_high: bool,
    ) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_sample_hold_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("trigger"),
            ComponentTraceValue::Bool(trigger_high),
        )
        .with_output(
            trace_key("held"),
            ComponentTraceValue::Float(f64::from(frame.held)),
        )
    }
}

impl Default for Ps3300SampleHold {
    fn default() -> Self {
        Self::new(Ps3300SampleHoldSettings::default())
    }
}

impl DiscreteComponent for Ps3300SampleHold {
    fn component_id(&self) -> Symbol {
        ps3_sample_hold_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_sample_hold_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_sample_hold_params()
    }

    fn reset(&mut self) {
        self.held = self.settings.initial_value;
        self.last_trigger_high = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(
                block.out_audio,
                frame,
                &[output.held, if output.captured { 1.0 } else { 0.0 }],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_sample_hold_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("held"), self.held.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for a [`Ps3300ExternalProcessor`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ExternalProcessorSettings {
    /// Gain applied to the incoming audio signal.
    pub audio_gain: f32,
    /// Gain applied to the incoming control voltage.
    pub cv_gain: f32,
    /// DC bias added to the processed control voltage, in volts.
    pub cv_bias_v: f32,
    /// CV at or above which the gate output reads high, in volts.
    pub gate_threshold_v: f32,
    /// Smoothing coefficient of the envelope follower, in 0.0..=1.0.
    pub follower_smoothing: f32,
}

impl Default for Ps3300ExternalProcessorSettings {
    fn default() -> Self {
        Self {
            audio_gain: 1.0,
            cv_gain: 1.0,
            cv_bias_v: 0.0,
            gate_threshold_v: 1.0,
            follower_smoothing: 0.2,
        }
    }
}

/// One rendered sample of the external processor's four output rails.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ExternalProcessorFrame {
    /// Gained and clamped audio output.
    pub audio: f32,
    /// Derived control voltage (gain, bias, and follower combined).
    pub cv: f32,
    /// Gate state from comparing `cv` against the threshold.
    pub gate: bool,
    /// Envelope-follower level of the audio magnitude.
    pub follower: f32,
}

/// External-signal processor: gains audio, follows its envelope, and derives a
/// control voltage and gate from an outside source.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300ExternalProcessor {
    settings: Ps3300ExternalProcessorSettings,
    follower: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300ExternalProcessor {
    /// Builds an external processor from the given (sanitized) settings.
    pub fn new(settings: Ps3300ExternalProcessorSettings) -> Self {
        Self {
            settings: sanitize_external(settings),
            follower: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Processes one sample of external audio and CV, updating the envelope
    /// follower and returning the audio, CV, gate, and follower rails.
    pub fn next_sample(&mut self, audio_in: f32, cv_in: f32) -> Ps3300ExternalProcessorFrame {
        let audio = (audio_in * self.settings.audio_gain).clamp(-4.0, 4.0);
        self.follower += (audio.abs() - self.follower) * self.settings.follower_smoothing;
        let cv = (cv_in * self.settings.cv_gain + self.settings.cv_bias_v + self.follower)
            .clamp(-10.0, 10.0);
        let frame = Ps3300ExternalProcessorFrame {
            audio,
            cv,
            gate: cv >= self.settings.gate_threshold_v,
            follower: self.follower,
        };
        self.last_trace = Some(self.external_trace(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn external_trace(&self, frame: Ps3300ExternalProcessorFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_external_processor_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(trace_key("gate"), ComponentTraceValue::Bool(frame.gate))
        .with_output(
            trace_key("cv"),
            ComponentTraceValue::Float(f64::from(frame.cv)),
        )
        .with_output(
            trace_key("follower"),
            ComponentTraceValue::Float(f64::from(frame.follower)),
        )
    }
}

impl Default for Ps3300ExternalProcessor {
    fn default() -> Self {
        Self::new(Ps3300ExternalProcessorSettings::default())
    }
}

impl DiscreteComponent for Ps3300ExternalProcessor {
    fn component_id(&self) -> Symbol {
        ps3_external_processor_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_external_processor_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_external_processor_params()
    }

    fn reset(&mut self) {
        self.follower = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(
                block.out_audio,
                frame,
                &[
                    output.audio,
                    output.cv,
                    if output.gate { 1.0 } else { 0.0 },
                    output.follower,
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_external_processor_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("follower"), self.follower.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component ids for the modulation family: generator, sample-hold,
/// and external processor.
pub fn ps3300_modulation_module_ids() -> [Symbol; 3] {
    [
        ps3_modulation_generator_component_id(),
        ps3_sample_hold_component_id(),
        ps3_external_processor_component_id(),
    ]
}

/// Returns the fixture names for the modulation conformance scenarios.
pub fn ps3300_modulation_fixture_names() -> [&'static str; 3] {
    PS3300_MODULATION_FIXTURE_NAMES
}

/// Returns the qualified component id for the modulation generator module.
pub fn ps3_modulation_generator_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-modulation-generator")
}

/// Returns the qualified component id for the sample-and-hold module.
pub fn ps3_sample_hold_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-sample-hold")
}

/// Returns the qualified component id for the external processor module.
pub fn ps3_external_processor_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-external-processor")
}

/// Returns the modulation generator's ports: rate CV input plus bipolar and
/// unipolar CV outputs.
pub fn ps3_modulation_generator_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("rate-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("unipolar-out", ComponentPortMedia::ControlVoltage).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the modulation generator's parameters: waveform, rate, and depth.
pub fn ps3_modulation_generator_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("waveform"),
            "Waveform",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                Ps3300ModulationWaveform::Sine.symbol(),
                Ps3300ModulationWaveform::Triangle.symbol(),
                Ps3300ModulationWaveform::Saw.symbol(),
                Ps3300ModulationWaveform::Square.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(param_key("rate-hz"), "Rate", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(0.01, 100.0, 5.0)),
        ComponentParamDescriptor::new(param_key("depth"), "Depth", ComponentParamUnit::Normalized)
            .with_range(ComponentParamRange::new(0.0, 1.0, 1.0)),
    ]
}

/// Returns the sample-and-hold's ports: signal and trigger inputs, held CV
/// output, and a capture gate.
pub fn ps3_sample_hold_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("signal-in", ComponentPortMedia::ControlVoltage),
        input_port("trigger-in", ComponentPortMedia::Gate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("capture-out", ComponentPortMedia::Gate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the sample-and-hold's parameters: initial value and trigger
/// threshold.
pub fn ps3_sample_hold_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("initial-value"),
            "Initial value",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("trigger-threshold-v"),
            "Trigger threshold",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 10.0, 0.5)),
    ]
}

/// Returns the external processor's ports: audio and CV inputs and the audio,
/// CV, gate, and follower outputs.
pub fn ps3_external_processor_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("follower-out", ComponentPortMedia::ControlVoltage).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the external processor's parameters: audio gain, CV gain, and gate
/// threshold.
pub fn ps3_external_processor_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("audio-gain"),
            "Audio gain",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("cv-gain"),
            "CV gain",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("gate-threshold-v"),
            "Gate threshold",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 1.0)),
    ]
}

fn sanitize_mg(settings: Ps3300ModulationGeneratorSettings) -> Ps3300ModulationGeneratorSettings {
    Ps3300ModulationGeneratorSettings {
        waveform: settings.waveform,
        rate_hz: settings.rate_hz.clamp(0.01, 100.0),
        depth: settings.depth.clamp(0.0, 1.0),
        offset: settings.offset.clamp(-1.0, 1.0),
        rate_cv_depth_octaves: settings.rate_cv_depth_octaves.clamp(-4.0, 4.0),
    }
}

fn sanitize_sample_hold(settings: Ps3300SampleHoldSettings) -> Ps3300SampleHoldSettings {
    Ps3300SampleHoldSettings {
        initial_value: settings.initial_value.clamp(-10.0, 10.0),
        trigger_threshold_v: settings.trigger_threshold_v.clamp(0.0, 10.0),
    }
}

fn sanitize_external(settings: Ps3300ExternalProcessorSettings) -> Ps3300ExternalProcessorSettings {
    Ps3300ExternalProcessorSettings {
        audio_gain: settings.audio_gain.clamp(0.0, 8.0),
        cv_gain: settings.cv_gain.clamp(0.0, 8.0),
        cv_bias_v: settings.cv_bias_v.clamp(-10.0, 10.0),
        gate_threshold_v: settings.gate_threshold_v.clamp(-10.0, 10.0),
        follower_smoothing: settings.follower_smoothing.clamp(0.0, 1.0),
    }
}

fn wave_sample(waveform: Ps3300ModulationWaveform, phase: f32) -> f32 {
    match waveform {
        Ps3300ModulationWaveform::Sine => (TAU * phase).sin(),
        Ps3300ModulationWaveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        Ps3300ModulationWaveform::Saw => 2.0 * phase - 1.0,
        Ps3300ModulationWaveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

fn input(channels: &[&[f32]], channel: usize, frame: usize) -> f32 {
    channels
        .get(channel)
        .and_then(|samples| samples.get(frame))
        .copied()
        .unwrap_or(0.0)
}

fn write_outputs(outputs: &mut [&mut [f32]], frame: usize, samples: &[f32]) {
    for (channel, output) in outputs.iter_mut().enumerate() {
        output[frame] = samples
            .get(channel)
            .copied()
            .or_else(|| samples.last().copied())
            .unwrap_or(0.0);
    }
}

fn input_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Input, 1)
}

fn output_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Output, 1)
}

fn port_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-port", name)
}

fn param_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-param", name)
}

fn inspect_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-inspect", name)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-trace", name)
}
