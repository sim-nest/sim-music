use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use super::common::{
    input, input_port, inspect_key, output_port, param_key, trace_key, write_outputs,
};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortMedia, ComponentPrepareConfig,
    ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Stage of the ADSR envelope generator's state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System700EnvelopeStage {
    /// Resting stage with the level held at zero.
    Idle,
    /// Rising stage advancing the level toward full.
    Attack,
    /// Falling stage advancing the level toward the sustain level.
    Decay,
    /// Held stage at the sustain level while the gate stays high.
    Sustain,
    /// Falling stage advancing the level back toward zero.
    Release,
}

impl System700EnvelopeStage {
    /// Returns the lowercase string name of the stage.
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

/// Configuration for the ADSR envelope generator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700EnvelopeSettings {
    /// Attack time in seconds to rise from zero to full level.
    pub attack_s: f32,
    /// Decay time in seconds to fall from full to the sustain level.
    pub decay_s: f32,
    /// Held level during the sustain stage.
    pub sustain_level: f32,
    /// Release time in seconds to fall from the current level to zero.
    pub release_s: f32,
    /// Output level scaling the envelope.
    pub level: f32,
}

impl Default for System700EnvelopeSettings {
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

/// ADSR envelope generator driven by a gate input.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Envelope {
    settings: System700EnvelopeSettings,
    sample_rate_hz: f32,
    stage: System700EnvelopeStage,
    level: f32,
    last_gate_high: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Envelope {
    /// Creates an envelope from sanitized settings at the default sample rate
    /// in the idle stage.
    pub fn new(settings: System700EnvelopeSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            stage: System700EnvelopeStage::Idle,
            level: 0.0,
            last_gate_high: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the processing sample rate in hertz, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the current envelope stage.
    pub fn stage(&self) -> System700EnvelopeStage {
        self.stage
    }

    /// Advances by one sample, updating the stage from the gate, and returns
    /// the scaled envelope level.
    pub fn next_sample(&mut self, gate_high: bool) -> f32 {
        if gate_high && !self.last_gate_high {
            self.stage = System700EnvelopeStage::Attack;
        } else if !gate_high && self.last_gate_high {
            self.stage = System700EnvelopeStage::Release;
        }
        self.last_gate_high = gate_high;
        self.advance_level();
        let output = self.level * self.settings.level;
        self.last_trace = Some(self.trace_frame(output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn advance_level(&mut self) {
        match self.stage {
            System700EnvelopeStage::Idle => self.level = 0.0,
            System700EnvelopeStage::Attack => {
                self.level =
                    step_toward(self.level, 1.0, self.settings.attack_s, self.sample_rate_hz);
                if self.level >= 1.0 {
                    self.stage = System700EnvelopeStage::Decay;
                }
            }
            System700EnvelopeStage::Decay => {
                self.level = step_toward(
                    self.level,
                    self.settings.sustain_level,
                    self.settings.decay_s,
                    self.sample_rate_hz,
                );
                if self.level <= self.settings.sustain_level {
                    self.stage = System700EnvelopeStage::Sustain;
                }
            }
            System700EnvelopeStage::Sustain => self.level = self.settings.sustain_level,
            System700EnvelopeStage::Release => {
                self.level = step_toward(
                    self.level,
                    0.0,
                    self.settings.release_s,
                    self.sample_rate_hz,
                );
                if self.level <= 0.0 {
                    self.stage = System700EnvelopeStage::Idle;
                }
            }
        }
    }

    fn trace_frame(&self, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_envelope_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("stage"),
            ComponentTraceValue::Text(self.stage.as_str().to_owned()),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Envelope {
    fn default() -> Self {
        Self::new(System700EnvelopeSettings::default())
    }
}

impl DiscreteComponent for System700Envelope {
    fn component_id(&self) -> Symbol {
        r700_envelope_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_envelope_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_envelope_params()
    }

    fn reset(&mut self) {
        self.stage = System700EnvelopeStage::Idle;
        self.level = 0.0;
        self.last_gate_high = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let sample = self.next_sample(input(block.in_audio, 0, frame) > 0.5);
            write_outputs(block.out_audio, frame, &[sample]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_envelope_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("stage"), self.stage.as_str().to_owned())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the stable component id for the envelope module.
pub fn r700_envelope_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-envelope")
}

/// Returns the port descriptors for the envelope module.
pub fn r700_envelope_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("gate-in", ComponentPortMedia::Gate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the envelope module.
pub fn r700_envelope_params() -> Vec<ComponentParamDescriptor> {
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

fn time_param(name: &'static str, label: &'static str, default: f64) -> ComponentParamDescriptor {
    ComponentParamDescriptor::new(param_key(name), label, ComponentParamUnit::Seconds)
        .with_range(ComponentParamRange::new(0.0, 20.0, default))
}

fn sanitize(settings: System700EnvelopeSettings) -> System700EnvelopeSettings {
    System700EnvelopeSettings {
        attack_s: settings.attack_s.clamp(0.0, 20.0),
        decay_s: settings.decay_s.clamp(0.0, 20.0),
        sustain_level: settings.sustain_level.clamp(0.0, 1.0),
        release_s: settings.release_s.clamp(0.0, 20.0),
        level: settings.level.clamp(0.0, 10.0),
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
