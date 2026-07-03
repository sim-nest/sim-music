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

/// Configuration for a [`System700VoltageProcessor`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700VoltageProcessorSettings {
    /// Scaling factor applied to the input control voltage.
    pub gain: f32,
    /// Constant offset in volts added after scaling.
    pub offset_v: f32,
    /// Whether the signal polarity is inverted before offset.
    pub invert: bool,
}

impl Default for System700VoltageProcessorSettings {
    fn default() -> Self {
        Self {
            gain: 1.0,
            offset_v: 0.0,
            invert: false,
        }
    }
}

/// Voltage processor that scales, inverts, and offsets a control voltage.
#[derive(Clone, Debug, PartialEq)]
pub struct System700VoltageProcessor {
    settings: System700VoltageProcessorSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700VoltageProcessor {
    /// Creates a voltage processor from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700VoltageProcessorSettings) -> Self {
        Self {
            settings: sanitize(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Applies gain, polarity, and offset to one `input` voltage, clamped to +/-10 V.
    pub fn transfer(&self, input: f32) -> f32 {
        let polarity = if self.settings.invert { -1.0 } else { 1.0 };
        (input * self.settings.gain * polarity + self.settings.offset_v).clamp(-10.0, 10.0)
    }

    /// Processes one `input` voltage and records a trace frame.
    pub fn next_sample(&mut self, input: f32) -> f32 {
        let output = self.transfer(input);
        self.last_trace = Some(self.trace_frame(output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn trace_frame(&self, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_voltage_processor_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700VoltageProcessor {
    fn default() -> Self {
        Self::new(System700VoltageProcessorSettings::default())
    }
}

impl DiscreteComponent for System700VoltageProcessor {
    fn component_id(&self) -> Symbol {
        r700_voltage_processor_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_voltage_processor_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_voltage_processor_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let sample = self.next_sample(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &[sample]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_voltage_processor_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("gain"), self.settings.gain.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 voltage processor module.
pub fn r700_voltage_processor_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-voltage-processor")
}

/// Returns the port descriptors for the System 700 voltage processor module.
pub fn r700_voltage_processor_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("cv-in", ComponentPortMedia::ControlVoltage),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 voltage processor module.
pub fn r700_voltage_processor_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("gain"), "Gain", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("offset-v"),
            "Offset",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 0.0)),
    ]
}

fn sanitize(settings: System700VoltageProcessorSettings) -> System700VoltageProcessorSettings {
    System700VoltageProcessorSettings {
        gain: settings.gain.clamp(0.0, 8.0),
        offset_v: settings.offset_v.clamp(-10.0, 10.0),
        invert: settings.invert,
    }
}
