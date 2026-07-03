use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    GateConvention, GateConverter,
};

/// Gain law applied by the M55 902 VCA to its control-voltage input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System55VcaResponse {
    /// Gain tracks the control voltage linearly.
    Linear,
    /// Gain tracks the square of the control voltage.
    Exponential,
    /// Linear gain followed by a saturating output stage.
    Saturated,
}

impl System55VcaResponse {
    /// Returns the lowercase identifier string for this response curve.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Exponential => "exponential",
            Self::Saturated => "saturated",
        }
    }

    /// Returns the qualified `audio-synth/m55-vca-response` symbol for this curve.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/m55-vca-response", self.as_str())
    }
}

/// Configuration for the M55 902 VCA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55VcaSettings {
    /// Gain law applied to the control voltage.
    pub response: System55VcaResponse,
    /// Maximum gain at full control voltage.
    pub gain: f32,
    /// Drive applied to the saturating stage when the response is saturated.
    pub saturation_drive: f32,
}

impl Default for System55VcaSettings {
    fn default() -> Self {
        Self {
            response: System55VcaResponse::Linear,
            gain: 1.0,
            saturation_drive: 2.0,
        }
    }
}

/// M55 902 voltage-controlled amplifier: scales an audio input by a control
/// voltage under the configured gain law.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Vca {
    settings: System55VcaSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Vca {
    /// Builds a VCA from sanitized settings.
    pub fn new(settings: System55VcaSettings) -> Self {
        Self {
            settings: sanitize_vca(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the gain for a control voltage, clamping it to `[0, 1]` and applying
    /// the configured response law and maximum gain.
    pub fn gain_for_cv(&self, cv: f32) -> f32 {
        let cv = cv.clamp(0.0, 1.0);
        let control = match self.settings.response {
            System55VcaResponse::Linear | System55VcaResponse::Saturated => cv,
            System55VcaResponse::Exponential => cv * cv,
        };
        control * self.settings.gain
    }

    /// Amplifies one input sample by the gain for `cv`, saturating when configured,
    /// records a trace frame, and advances the clock.
    pub fn next_sample(&mut self, input: f32, cv: f32) -> f32 {
        let gain = self.gain_for_cv(cv);
        let raw = input * gain;
        let output = match self.settings.response {
            System55VcaResponse::Saturated => saturate(raw, self.settings.saturation_drive),
            System55VcaResponse::Linear | System55VcaResponse::Exponential => raw.clamp(-4.0, 4.0),
        };
        self.last_trace = Some(trace_output(m55_vca_component_id(), self.clock, output));
        self.clock = self.clock.saturating_add(1);
        output
    }
}

impl Default for System55Vca {
    fn default() -> Self {
        Self::new(System55VcaSettings::default())
    }
}

impl DiscreteComponent for System55Vca {
    fn component_id(&self) -> Symbol {
        m55_vca_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_vca_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_vca_params()
    }

    fn reset(&mut self) {
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
        ComponentInspection::new(m55_vca_component_id(), ComponentBackend::Algorithmic, true)
            .with_field(
                inspect_key("response"),
                self.settings.response.as_str().to_owned(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Stage of the M55 911 envelope generator's attack/decay/sustain/release cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System55EnvelopeStage {
    /// Envelope at rest, level held at zero.
    Idle,
    /// Rising toward full level.
    Attack,
    /// Falling from full level toward the sustain level.
    Decay,
    /// Holding at the sustain level while the gate is active.
    Sustain,
    /// Falling toward zero after the gate releases.
    Release,
}

impl System55EnvelopeStage {
    /// Returns the lowercase identifier string for this stage.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Attack => "attack",
            Self::Decay => "decay",
            Self::Sustain => "sustain",
            Self::Release => "release",
        }
    }
}

/// Configuration for the M55 911 envelope generator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55EnvelopeSettings {
    /// Attack time in seconds.
    pub attack_s: f32,
    /// Decay time in seconds.
    pub decay_s: f32,
    /// Sustain level in `[0, 1]`.
    pub sustain_level: f32,
    /// Release time in seconds.
    pub release_s: f32,
    /// Overall output level scaling the envelope.
    pub level: f32,
}

impl Default for System55EnvelopeSettings {
    fn default() -> Self {
        Self {
            attack_s: 0.01,
            decay_s: 0.1,
            sustain_level: 0.7,
            release_s: 0.2,
            level: 1.0,
        }
    }
}

/// M55 911 envelope generator: an S-trigger-gated ADSR contour driving a control
/// voltage output.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Envelope {
    settings: System55EnvelopeSettings,
    gate: GateConverter,
    sample_rate_hz: f32,
    stage: System55EnvelopeStage,
    level: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Envelope {
    /// Builds an envelope generator from sanitized settings.
    pub fn new(settings: System55EnvelopeSettings) -> Self {
        Self {
            settings: sanitize_envelope(settings),
            gate: GateConverter::new(GateConvention::s_trigger()),
            sample_rate_hz: 48_000.0,
            stage: System55EnvelopeStage::Idle,
            level: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the sample rate used for stage timing, flooring it at 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the current envelope stage.
    pub fn stage(&self) -> System55EnvelopeStage {
        self.stage
    }

    /// Advances the envelope by one sample for the given S-trigger voltage,
    /// updating the stage, recording a trace frame, and returning the level.
    pub fn next_sample(&mut self, s_trigger_v: f32) -> f32 {
        let gate = self.gate.convert(s_trigger_v);
        if gate.triggered {
            self.stage = System55EnvelopeStage::Attack;
        } else if !gate.active && self.stage != System55EnvelopeStage::Idle {
            self.stage = System55EnvelopeStage::Release;
        }
        self.advance_level();
        let output = self.level * self.settings.level;
        self.last_trace = Some(trace_state_output(
            m55_envelope_component_id(),
            self.clock,
            "stage",
            self.stage.as_str(),
            output,
        ));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn advance_level(&mut self) {
        match self.stage {
            System55EnvelopeStage::Idle => self.level = 0.0,
            System55EnvelopeStage::Attack => {
                self.level =
                    step_toward(self.level, 1.0, self.settings.attack_s, self.sample_rate_hz);
                if self.level >= 1.0 {
                    self.stage = System55EnvelopeStage::Decay;
                }
            }
            System55EnvelopeStage::Decay => {
                self.level = step_toward(
                    self.level,
                    self.settings.sustain_level,
                    self.settings.decay_s,
                    self.sample_rate_hz,
                );
                if self.level <= self.settings.sustain_level {
                    self.stage = System55EnvelopeStage::Sustain;
                }
            }
            System55EnvelopeStage::Sustain => self.level = self.settings.sustain_level,
            System55EnvelopeStage::Release => {
                self.level = step_toward(
                    self.level,
                    0.0,
                    self.settings.release_s,
                    self.sample_rate_hz,
                );
                if self.level <= 0.0 {
                    self.stage = System55EnvelopeStage::Idle;
                }
            }
        }
    }
}

impl Default for System55Envelope {
    fn default() -> Self {
        Self::new(System55EnvelopeSettings::default())
    }
}

impl DiscreteComponent for System55Envelope {
    fn component_id(&self) -> Symbol {
        m55_envelope_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_envelope_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_envelope_params()
    }

    fn reset(&mut self) {
        self.gate.reset();
        self.stage = System55EnvelopeStage::Idle;
        self.level = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let sample = self.next_sample(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &[sample]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_envelope_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("stage"), self.stage.as_str().to_owned())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for the M55 911A dual trigger delay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55TriggerDelaySettings {
    /// Delay between input trigger and output pulse, in seconds.
    pub delay_s: f32,
    /// Width of the emitted output pulse, in seconds.
    pub pulse_s: f32,
}

impl Default for System55TriggerDelaySettings {
    fn default() -> Self {
        Self {
            delay_s: 0.05,
            pulse_s: 0.01,
        }
    }
}

/// One output frame from the trigger delay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55TriggerDelayFrame {
    /// Whether the output pulse is currently high.
    pub active: bool,
    /// Whether the pulse just went high on this frame (rising edge).
    pub triggered: bool,
    /// Native S-trigger voltage for the current pulse state.
    pub s_trigger_v: f32,
}

/// M55 911A dual trigger delay: delays an incoming S-trigger by a set time, then
/// emits a fixed-width output pulse.
#[derive(Clone, Debug, PartialEq)]
pub struct System55TriggerDelay {
    settings: System55TriggerDelaySettings,
    gate: GateConverter,
    sample_rate_hz: f32,
    delay_remaining: usize,
    pulse_remaining: usize,
    was_active: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55TriggerDelay {
    /// Builds a trigger delay from sanitized settings.
    pub fn new(settings: System55TriggerDelaySettings) -> Self {
        Self {
            settings: sanitize_trigger_delay(settings),
            gate: GateConverter::new(GateConvention::s_trigger()),
            sample_rate_hz: 48_000.0,
            delay_remaining: 0,
            pulse_remaining: 0,
            was_active: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the sample rate used to convert delay and pulse times to samples,
    /// flooring it at 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Advances the delay by one sample for the given S-trigger voltage and returns
    /// the resulting output frame.
    pub fn next_frame(&mut self, s_trigger_v: f32) -> System55TriggerDelayFrame {
        if self.gate.convert(s_trigger_v).triggered {
            self.delay_remaining = seconds_to_samples(self.settings.delay_s, self.sample_rate_hz);
            if self.delay_remaining == 0 {
                self.pulse_remaining =
                    seconds_to_samples(self.settings.pulse_s, self.sample_rate_hz);
            }
        }
        if self.delay_remaining > 0 {
            self.delay_remaining -= 1;
            if self.delay_remaining == 0 {
                self.pulse_remaining =
                    seconds_to_samples(self.settings.pulse_s, self.sample_rate_hz);
            }
        }
        let active = self.pulse_remaining > 0;
        if self.pulse_remaining > 0 {
            self.pulse_remaining -= 1;
        }
        let frame = System55TriggerDelayFrame {
            active,
            triggered: active && !self.was_active,
            s_trigger_v: GateConvention::s_trigger().native_voltage(active),
        };
        self.was_active = active;
        self.last_trace = Some(trace_bool_output(
            m55_trigger_delay_component_id(),
            self.clock,
            frame.active,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }
}

impl Default for System55TriggerDelay {
    fn default() -> Self {
        Self::new(System55TriggerDelaySettings::default())
    }
}

impl DiscreteComponent for System55TriggerDelay {
    fn component_id(&self) -> Symbol {
        m55_trigger_delay_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_trigger_delay_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_trigger_delay_params()
    }

    fn reset(&mut self) {
        self.gate.reset();
        self.delay_remaining = 0;
        self.pulse_remaining = 0;
        self.was_active = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &[output.s_trigger_v]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_trigger_delay_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("delay-s"), self.settings.delay_s.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the qualified module id for the M55 902 VCA.
pub fn m55_vca_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-902-vca")
}

/// Returns the qualified module id for the M55 911 envelope generator.
pub fn m55_envelope_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-911-envelope-generator")
}

/// Returns the qualified module id for the M55 911A dual trigger delay.
pub fn m55_trigger_delay_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-911a-dual-trigger-delay")
}

/// Returns the port descriptors for the M55 902 VCA.
pub fn m55_vca_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("gain-cv-in", ComponentPortMedia::ControlVoltage),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the M55 902 VCA.
pub fn m55_vca_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("response"),
            "Response",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                System55VcaResponse::Linear.symbol(),
                System55VcaResponse::Exponential.symbol(),
                System55VcaResponse::Saturated.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(param_key("gain"), "Gain", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(0.0, 4.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("saturation-drive"),
            "Saturation drive",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.5, 12.0, 2.0)),
    ]
}

/// Returns the port descriptors for the M55 911 envelope generator.
pub fn m55_envelope_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("s-trigger-in", ComponentPortMedia::Gate),
        output_port("envelope-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the M55 911 envelope generator.
pub fn m55_envelope_params() -> Vec<ComponentParamDescriptor> {
    vec![
        time_param("attack-s", "Attack", 0.01),
        time_param("decay-s", "Decay", 0.1),
        ComponentParamDescriptor::new(
            param_key("sustain-level"),
            "Sustain",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.7)),
        time_param("release-s", "Release", 0.2),
    ]
}

/// Returns the port descriptors for the M55 911A dual trigger delay.
pub fn m55_trigger_delay_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("s-trigger-in", ComponentPortMedia::Gate),
        output_port("s-trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the M55 911A dual trigger delay.
pub fn m55_trigger_delay_params() -> Vec<ComponentParamDescriptor> {
    vec![
        time_param("delay-s", "Delay", 0.05),
        time_param("pulse-s", "Pulse", 0.01),
    ]
}

fn sanitize_vca(settings: System55VcaSettings) -> System55VcaSettings {
    System55VcaSettings {
        response: settings.response,
        gain: settings.gain.clamp(0.0, 4.0),
        saturation_drive: settings.saturation_drive.clamp(0.5, 12.0),
    }
}

fn sanitize_envelope(settings: System55EnvelopeSettings) -> System55EnvelopeSettings {
    System55EnvelopeSettings {
        attack_s: settings.attack_s.clamp(0.0, 20.0),
        decay_s: settings.decay_s.clamp(0.0, 20.0),
        sustain_level: settings.sustain_level.clamp(0.0, 1.0),
        release_s: settings.release_s.clamp(0.0, 20.0),
        level: settings.level.clamp(0.0, 10.0),
    }
}

fn sanitize_trigger_delay(settings: System55TriggerDelaySettings) -> System55TriggerDelaySettings {
    System55TriggerDelaySettings {
        delay_s: settings.delay_s.clamp(0.0, 20.0),
        pulse_s: settings.pulse_s.clamp(0.0, 20.0),
    }
}

fn step_toward(current: f32, target: f32, seconds: f32, sample_rate_hz: f32) -> f32 {
    if seconds <= 0.0 {
        return target;
    }
    let step = 1.0 / (seconds * sample_rate_hz).max(1.0);
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

fn seconds_to_samples(seconds: f32, sample_rate_hz: f32) -> usize {
    (seconds * sample_rate_hz).round().max(0.0) as usize
}

fn saturate(input: f32, drive: f32) -> f32 {
    let scale = drive.tanh().max(0.001);
    ((input * drive).tanh() / scale).clamp(-1.0, 1.0)
}

fn trace_state_output(
    id: Symbol,
    clock: u64,
    state_name: &'static str,
    state: &'static str,
    output: f32,
) -> ComponentTraceFrame {
    ComponentTraceFrame::new(id, ComponentBackend::Algorithmic, clock)
        .with_state(
            trace_key(state_name),
            ComponentTraceValue::Text(state.to_owned()),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
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

fn trace_bool_output(id: Symbol, clock: u64, output: bool) -> ComponentTraceFrame {
    ComponentTraceFrame::new(id, ComponentBackend::Algorithmic, clock)
        .with_output(trace_key("output"), ComponentTraceValue::Bool(output))
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
