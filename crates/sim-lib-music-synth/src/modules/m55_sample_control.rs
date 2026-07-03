use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    GateConvention, GateConverter,
};

/// Configuration for the System 55 envelope follower.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55EnvelopeFollowerSettings {
    /// Attack time constant in seconds used while the envelope is rising.
    pub attack_s: f32,
    /// Release time constant in seconds used while the envelope is falling.
    pub release_s: f32,
    /// Envelope level at or above which the gate output is asserted.
    pub gate_threshold: f32,
}

impl Default for System55EnvelopeFollowerSettings {
    fn default() -> Self {
        Self {
            attack_s: 0.01,
            release_s: 0.15,
            gate_threshold: 0.1,
        }
    }
}

/// Per-sample output of the System 55 envelope follower.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55EnvelopeFollowerFrame {
    /// Smoothed envelope magnitude tracked from the rectified input.
    pub envelope: f32,
    /// Whether the envelope currently sits at or above the gate threshold.
    pub gate: bool,
    /// Whether the gate just transitioned from low to high this sample.
    pub trigger: bool,
}

/// System 55 envelope follower that tracks the magnitude of an audio input
/// and derives a gate and trigger from it.
#[derive(Clone, Debug, PartialEq)]
pub struct System55EnvelopeFollower {
    settings: System55EnvelopeFollowerSettings,
    sample_rate_hz: f32,
    envelope: f32,
    last_gate: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55EnvelopeFollower {
    /// Creates a follower from sanitized settings at the default sample rate.
    pub fn new(settings: System55EnvelopeFollowerSettings) -> Self {
        Self {
            settings: sanitize_env_follower(settings),
            sample_rate_hz: 48_000.0,
            envelope: 0.0,
            last_gate: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the processing sample rate in hertz, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Advances the follower by one sample and returns the resulting frame.
    pub fn next_frame(&mut self, input: f32) -> System55EnvelopeFollowerFrame {
        let target = input.abs().clamp(0.0, 10.0);
        let seconds = if target > self.envelope {
            self.settings.attack_s
        } else {
            self.settings.release_s
        };
        self.envelope = smooth_toward(self.envelope, target, seconds, self.sample_rate_hz);
        let gate = self.envelope >= self.settings.gate_threshold;
        let frame = System55EnvelopeFollowerFrame {
            envelope: self.envelope,
            gate,
            trigger: gate && !self.last_gate,
        };
        self.last_gate = gate;
        self.last_trace = Some(trace_output(
            m55_env_follower_component_id(),
            self.clock,
            self.envelope,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }
}

impl Default for System55EnvelopeFollower {
    fn default() -> Self {
        Self::new(System55EnvelopeFollowerSettings::default())
    }
}

impl DiscreteComponent for System55EnvelopeFollower {
    fn component_id(&self) -> Symbol {
        m55_env_follower_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_env_follower_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_env_follower_params()
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.last_gate = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(input(block.in_audio, 0, frame));
            write_outputs(
                block.out_audio,
                frame,
                &[output.envelope, if output.gate { 1.0 } else { 0.0 }],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_env_follower_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("envelope"), self.envelope.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for the System 55 sample-and-hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55SampleHoldSettings {
    /// Value held before the first trigger arrives.
    pub initial_value: f32,
}

impl Default for System55SampleHoldSettings {
    fn default() -> Self {
        Self { initial_value: 0.0 }
    }
}

/// System 55 sample-and-hold that latches its signal input whenever the
/// S-trigger input fires.
#[derive(Clone, Debug, PartialEq)]
pub struct System55SampleHold {
    settings: System55SampleHoldSettings,
    trigger: GateConverter,
    held: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55SampleHold {
    /// Creates a sample-and-hold whose held value starts at the clamped
    /// initial value.
    pub fn new(settings: System55SampleHoldSettings) -> Self {
        let settings = System55SampleHoldSettings {
            initial_value: settings.initial_value.clamp(-10.0, 10.0),
        };
        Self {
            held: settings.initial_value,
            settings,
            trigger: GateConverter::new(GateConvention::s_trigger()),
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the currently held value.
    pub fn held_value(&self) -> f32 {
        self.held
    }

    /// Advances by one sample, latching `input` when `s_trigger_v` fires, and
    /// returns the held value.
    pub fn next_sample(&mut self, input: f32, s_trigger_v: f32) -> f32 {
        if self.trigger.convert(s_trigger_v).triggered {
            self.held = input.clamp(-10.0, 10.0);
        }
        self.last_trace = Some(trace_output(
            m55_sample_hold_component_id(),
            self.clock,
            self.held,
        ));
        self.clock = self.clock.saturating_add(1);
        self.held
    }
}

impl Default for System55SampleHold {
    fn default() -> Self {
        Self::new(System55SampleHoldSettings::default())
    }
}

impl DiscreteComponent for System55SampleHold {
    fn component_id(&self) -> Symbol {
        m55_sample_hold_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_sample_hold_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_sample_hold_params()
    }

    fn reset(&mut self) {
        self.trigger.reset();
        self.held = self.settings.initial_value;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let sample = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(block.out_audio, frame, &[sample]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_sample_hold_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("held"), self.held.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for the System 55 sequential controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55SequencerSettings {
    /// Control-voltage value emitted at each of the eight steps.
    pub steps: [f32; 8],
    /// Number of active steps before the sequence wraps, from 1 to 8.
    pub step_count: usize,
    /// Bitmask selecting which steps assert their gate output.
    pub gate_mask: u16,
}

impl Default for System55SequencerSettings {
    fn default() -> Self {
        Self {
            steps: [0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25],
            step_count: 8,
            gate_mask: 0xff,
        }
    }
}

/// Per-sample output of the System 55 sequencer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55SequencerFrame {
    /// Index of the currently selected step.
    pub step: usize,
    /// Control voltage for the current step.
    pub cv: f32,
    /// Whether the current step's gate bit is set.
    pub gate: bool,
    /// Whether a clock or reset advance occurred this sample.
    pub trigger: bool,
}

/// System 55 sequential controller that walks a ring of step voltages under
/// clock and reset triggers.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Sequencer {
    settings: System55SequencerSettings,
    clock_gate: GateConverter,
    reset_gate: GateConverter,
    step: usize,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Sequencer {
    /// Creates a sequencer from sanitized settings, starting at step 0.
    pub fn new(settings: System55SequencerSettings) -> Self {
        Self {
            settings: sanitize_sequencer(settings),
            clock_gate: GateConverter::new(GateConvention::s_trigger()),
            reset_gate: GateConverter::new(GateConvention::s_trigger()),
            step: 0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Advances by one sample, applying reset before clock, and returns the
    /// resulting step frame.
    pub fn next_frame(
        &mut self,
        clock_s_trigger_v: f32,
        reset_s_trigger_v: f32,
    ) -> System55SequencerFrame {
        let reset = self.reset_gate.convert(reset_s_trigger_v);
        let clock = self.clock_gate.convert(clock_s_trigger_v);
        if reset.triggered {
            self.step = 0;
        } else if clock.triggered {
            self.step = (self.step + 1) % self.settings.step_count;
        }
        let frame = System55SequencerFrame {
            step: self.step,
            cv: self.settings.steps[self.step],
            gate: (self.settings.gate_mask & (1 << self.step)) != 0,
            trigger: reset.triggered || clock.triggered,
        };
        self.last_trace = Some(trace_output(
            m55_sequencer_component_id(),
            self.clock,
            frame.cv,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }
}

impl Default for System55Sequencer {
    fn default() -> Self {
        Self::new(System55SequencerSettings::default())
    }
}

impl DiscreteComponent for System55Sequencer {
    fn component_id(&self) -> Symbol {
        m55_sequencer_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_sequencer_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_sequencer_params()
    }

    fn reset(&mut self) {
        self.clock_gate.reset();
        self.reset_gate.reset();
        self.step = 0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(
                block.out_audio,
                frame,
                &[
                    output.cv,
                    GateConvention::s_trigger().native_voltage(output.gate),
                    GateConvention::s_trigger().native_voltage(output.trigger),
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_sequencer_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("step"), self.step.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the stable component id for the envelope follower module.
pub fn m55_env_follower_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-912-envelope-follower")
}

/// Returns the stable component id for the sample-and-hold module.
pub fn m55_sample_hold_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-928-sample-hold")
}

/// Returns the stable component id for the sequential controller module.
pub fn m55_sequencer_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-960-sequential-controller")
}

/// Returns the port descriptors for the envelope follower module.
pub fn m55_env_follower_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        output_port("envelope-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the envelope follower module.
pub fn m55_env_follower_params() -> Vec<ComponentParamDescriptor> {
    vec![
        time_param("attack-s", "Attack", 0.01),
        time_param("release-s", "Release", 0.15),
        ComponentParamDescriptor::new(
            param_key("gate-threshold"),
            "Gate threshold",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 10.0, 0.1)),
    ]
}

/// Returns the port descriptors for the sample-and-hold module.
pub fn m55_sample_hold_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("signal-in", ComponentPortMedia::ControlVoltage),
        input_port("s-trigger-in", ComponentPortMedia::Gate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the sample-and-hold module.
pub fn m55_sample_hold_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("initial-value"),
            "Initial value",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 0.0)),
    ]
}

/// Returns the port descriptors for the sequential controller module.
pub fn m55_sequencer_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("s-trigger-in", ComponentPortMedia::Gate),
        input_port("reset-in", ComponentPortMedia::Gate).optional(),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("s-trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the sequential controller module.
pub fn m55_sequencer_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("step-count"),
            "Step count",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(8),
        ComponentParamDescriptor::new(
            param_key("gate-mask"),
            "Gate mask",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(0xff),
    ]
}

fn sanitize_env_follower(
    settings: System55EnvelopeFollowerSettings,
) -> System55EnvelopeFollowerSettings {
    System55EnvelopeFollowerSettings {
        attack_s: settings.attack_s.clamp(0.0, 20.0),
        release_s: settings.release_s.clamp(0.0, 20.0),
        gate_threshold: settings.gate_threshold.clamp(0.0, 10.0),
    }
}

fn sanitize_sequencer(settings: System55SequencerSettings) -> System55SequencerSettings {
    System55SequencerSettings {
        steps: settings.steps.map(|step| step.clamp(-10.0, 10.0)),
        step_count: settings.step_count.clamp(1, 8),
        gate_mask: settings.gate_mask & 0xff,
    }
}

fn smooth_toward(current: f32, target: f32, seconds: f32, sample_rate_hz: f32) -> f32 {
    if seconds <= 0.0 {
        return target;
    }
    let coefficient = 1.0 - (-1.0 / (seconds * sample_rate_hz).max(1.0)).exp();
    current + coefficient * (target - current)
}

fn time_param(name: &'static str, label: &'static str, default: f64) -> ComponentParamDescriptor {
    ComponentParamDescriptor::new(param_key(name), label, ComponentParamUnit::Seconds)
        .with_range(ComponentParamRange::new(0.0, 20.0, default))
}

fn trace_output(id: Symbol, clock: u64, output: f32) -> ComponentTraceFrame {
    ComponentTraceFrame::new(id, ComponentBackend::Algorithmic, clock).with_output(
        trace_key("output"),
        ComponentTraceValue::Float(f64::from(output)),
    )
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
    Symbol::qualified("audio-synth/m55-port", name)
}

fn param_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/m55-param", name)
}

fn inspect_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/m55-inspect", name)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/m55-trace", name)
}
