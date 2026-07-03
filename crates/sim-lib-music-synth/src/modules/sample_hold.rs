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

/// Configuration for a [`System700SampleHold`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700SampleHoldSettings {
    /// Value held before the first sampling trigger and after reset.
    pub initial_value: f32,
    /// Trigger level at or above which a new sample is latched.
    pub trigger_threshold: f32,
}

impl Default for System700SampleHoldSettings {
    fn default() -> Self {
        Self {
            initial_value: 0.0,
            trigger_threshold: 0.5,
        }
    }
}

/// Sample-and-hold that latches its input on each rising trigger edge.
#[derive(Clone, Debug, PartialEq)]
pub struct System700SampleHold {
    settings: System700SampleHoldSettings,
    held: f32,
    last_trigger_high: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700SampleHold {
    /// Creates a sample-and-hold from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700SampleHoldSettings) -> Self {
        let settings = sanitize(settings);
        Self {
            held: settings.initial_value,
            settings,
            last_trigger_high: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the currently held value.
    pub fn held_value(&self) -> f32 {
        self.held
    }

    /// Latches `input` on a rising `trigger` edge, otherwise returns the held value.
    pub fn next_sample(&mut self, input: f32, trigger: f32) -> f32 {
        let trigger_high = trigger >= self.settings.trigger_threshold;
        if trigger_high && !self.last_trigger_high {
            self.held = input.clamp(-10.0, 10.0);
        }
        self.last_trigger_high = trigger_high;
        self.last_trace = Some(self.trace_frame(self.held, trigger_high));
        self.clock = self.clock.saturating_add(1);
        self.held
    }

    fn trace_frame(&self, output: f32, trigger_high: bool) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_sample_hold_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("trigger"),
            ComponentTraceValue::Bool(trigger_high),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700SampleHold {
    fn default() -> Self {
        Self::new(System700SampleHoldSettings::default())
    }
}

impl DiscreteComponent for System700SampleHold {
    fn component_id(&self) -> Symbol {
        r700_sample_hold_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_sample_hold_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_sample_hold_params()
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
            let sample = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(block.out_audio, frame, &[sample]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_sample_hold_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("held"), self.held.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 sample-and-hold module.
pub fn r700_sample_hold_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-sample-hold")
}

/// Returns the port descriptors for the System 700 sample-and-hold module.
pub fn r700_sample_hold_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("signal-in", ComponentPortMedia::ControlVoltage),
        input_port("trigger-in", ComponentPortMedia::Gate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 sample-and-hold module.
pub fn r700_sample_hold_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("initial-value"),
            "Initial value",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("trigger-threshold"),
            "Trigger threshold",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 10.0, 0.5)),
    ]
}

fn sanitize(settings: System700SampleHoldSettings) -> System700SampleHoldSettings {
    System700SampleHoldSettings {
        initial_value: settings.initial_value.clamp(-10.0, 10.0),
        trigger_threshold: settings.trigger_threshold.clamp(0.0, 10.0),
    }
}
