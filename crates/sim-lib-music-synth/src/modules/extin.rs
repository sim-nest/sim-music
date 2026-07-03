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

/// Configuration for a [`System700ExternalInput`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700ExternalInputSettings {
    /// Gain applied to the incoming line-level signal.
    pub gain: f32,
    /// Bias in volts added to the derived control voltage.
    pub cv_bias_v: f32,
    /// Control-voltage threshold in volts at or above which the gate opens.
    pub gate_threshold_v: f32,
}

impl Default for System700ExternalInputSettings {
    fn default() -> Self {
        Self {
            gain: 1.0,
            cv_bias_v: 0.0,
            gate_threshold_v: 1.0,
        }
    }
}

/// Outputs derived from one external-input sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System700ExternalInputFrame {
    /// Gain-scaled audio signal.
    pub audio: f32,
    /// Envelope-follower-style control voltage derived from the input.
    pub cv: f32,
    /// Whether the derived control voltage crossed the gate threshold.
    pub gate: bool,
}

/// External-input module that conditions a line-level signal into audio, CV, and gate.
#[derive(Clone, Debug, PartialEq)]
pub struct System700ExternalInput {
    settings: System700ExternalInputSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700ExternalInput {
    /// Creates an external-input module from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700ExternalInputSettings) -> Self {
        Self {
            settings: sanitize(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Maps one raw `input` sample into audio, control-voltage, and gate outputs.
    pub fn map_input(&self, input: f32) -> System700ExternalInputFrame {
        let scaled = input * self.settings.gain;
        let cv = (scaled + self.settings.cv_bias_v).clamp(-10.0, 10.0);
        System700ExternalInputFrame {
            audio: scaled.clamp(-4.0, 4.0),
            cv,
            gate: cv >= self.settings.gate_threshold_v,
        }
    }

    /// Maps one `input` sample and records a trace frame.
    pub fn next_frame(&mut self, input: f32) -> System700ExternalInputFrame {
        let frame = self.map_input(input);
        self.last_trace = Some(self.trace_frame(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: System700ExternalInputFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_external_input_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(trace_key("gate"), ComponentTraceValue::Bool(frame.gate))
        .with_output(
            trace_key("cv"),
            ComponentTraceValue::Float(f64::from(frame.cv)),
        )
    }
}

impl Default for System700ExternalInput {
    fn default() -> Self {
        Self::new(System700ExternalInputSettings::default())
    }
}

impl DiscreteComponent for System700ExternalInput {
    fn component_id(&self) -> Symbol {
        r700_external_input_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_external_input_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_external_input_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let mapped = self.next_frame(input(block.in_audio, 0, frame));
            write_outputs(
                block.out_audio,
                frame,
                &[mapped.audio, mapped.cv, if mapped.gate { 1.0 } else { 0.0 }],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_external_input_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("gain"), self.settings.gain.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 external-input module.
pub fn r700_external_input_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-external-input")
}

/// Returns the port descriptors for the System 700 external-input module.
pub fn r700_external_input_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("line-in", ComponentPortMedia::AudioRate),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 external-input module.
pub fn r700_external_input_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("gain"), "Gain", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("cv-bias-v"),
            "CV bias",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("gate-threshold-v"),
            "Gate threshold",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 1.0)),
    ]
}

fn sanitize(settings: System700ExternalInputSettings) -> System700ExternalInputSettings {
    System700ExternalInputSettings {
        gain: settings.gain.clamp(0.0, 8.0),
        cv_bias_v: settings.cv_bias_v.clamp(-10.0, 10.0),
        gate_threshold_v: settings.gate_threshold_v.clamp(-10.0, 10.0),
    }
}
