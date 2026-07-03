use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use super::common::{input, input_port, inspect_key, output_port, trace_key, write_outputs};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentPortDescriptor,
    ComponentPortMedia, ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue,
    DiscreteComponent,
};

/// Configuration for a [`System700Multiple`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct System700MultipleSettings {
    /// Number of active fan-out copies (1..=4).
    pub output_count: usize,
}

impl Default for System700MultipleSettings {
    fn default() -> Self {
        Self { output_count: 4 }
    }
}

/// Multiple (mult) that fans one input signal out to several buffered copies.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Multiple {
    settings: System700MultipleSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Multiple {
    /// Creates a multiple from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700MultipleSettings) -> Self {
        Self {
            settings: sanitize(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Copies the clamped `input` to all four output taps.
    pub fn fanout(&self, input: f32) -> [f32; 4] {
        [input.clamp(-10.0, 10.0); 4]
    }

    /// Fans out one `input` sample and records a trace frame.
    pub fn next_samples(&mut self, input: f32) -> [f32; 4] {
        let output = self.fanout(input);
        self.last_trace = Some(self.trace_frame(output[0]));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn trace_frame(&self, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_multiple_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Multiple {
    fn default() -> Self {
        Self::new(System700MultipleSettings::default())
    }
}

impl DiscreteComponent for System700Multiple {
    fn component_id(&self) -> Symbol {
        r700_multiple_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_multiple_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        Vec::new()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let samples = self.next_samples(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &samples);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_multiple_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("outputs"),
            self.settings.output_count.to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 multiple module.
pub fn r700_multiple_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-multiple")
}

/// Returns the port descriptors for the System 700 multiple module.
pub fn r700_multiple_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("signal-in", ComponentPortMedia::ControlVoltage),
        output_port("out-1", ComponentPortMedia::ControlVoltage),
        output_port("out-2", ComponentPortMedia::ControlVoltage),
        output_port("out-3", ComponentPortMedia::ControlVoltage),
        output_port("out-4", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 multiple module (none).
pub fn r700_multiple_params() -> Vec<ComponentParamDescriptor> {
    Vec::new()
}

fn sanitize(settings: System700MultipleSettings) -> System700MultipleSettings {
    System700MultipleSettings {
        output_count: settings.output_count.clamp(1, 4),
    }
}
