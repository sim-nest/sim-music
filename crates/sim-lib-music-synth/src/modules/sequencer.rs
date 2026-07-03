use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use super::common::{
    input, input_port, inspect_key, output_port, param_key, trace_key, write_outputs,
};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamUnit,
    ComponentPortDescriptor, ComponentPortMedia, ComponentPrepareConfig, ComponentTraceFrame,
    ComponentTraceValue, DiscreteComponent,
};

/// Configuration for a [`System700Sequencer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700SequencerSettings {
    /// Control-voltage value emitted at each of the eight steps.
    pub steps: [f32; 8],
    /// Number of active steps before the pattern wraps (1..=8).
    pub step_count: usize,
    /// Bitmask selecting which steps open the gate output.
    pub gate_mask: u16,
}

impl Default for System700SequencerSettings {
    fn default() -> Self {
        Self {
            steps: [0.0, 0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25],
            step_count: 8,
            gate_mask: 0xff,
        }
    }
}

/// One step's outputs produced by a [`System700Sequencer`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System700SequencerFrame {
    /// Zero-based index of the active step.
    pub step: usize,
    /// Control voltage of the active step.
    pub cv: f32,
    /// Whether the active step's gate is open.
    pub gate: bool,
    /// Whether this frame advanced to a new step (clock or reset edge).
    pub trigger: bool,
}

/// Eight-step analog-style sequencer clocked by gate edges.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Sequencer {
    settings: System700SequencerSettings,
    step: usize,
    last_clock: bool,
    last_reset: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Sequencer {
    /// Creates a sequencer from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700SequencerSettings) -> Self {
        Self {
            settings: sanitize(settings),
            step: 0,
            last_clock: false,
            last_reset: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the index of the currently active step.
    pub fn step(&self) -> usize {
        self.step
    }

    /// Advances on a rising clock edge (or resets on a rising reset edge) and returns the step frame.
    pub fn next_frame(&mut self, clock_high: bool, reset_high: bool) -> System700SequencerFrame {
        let reset_edge = reset_high && !self.last_reset;
        let clock_edge = clock_high && !self.last_clock;
        if reset_edge {
            self.step = 0;
        } else if clock_edge {
            self.step = (self.step + 1) % self.settings.step_count;
        }
        self.last_clock = clock_high;
        self.last_reset = reset_high;
        let frame = System700SequencerFrame {
            step: self.step,
            cv: self.settings.steps[self.step],
            gate: (self.settings.gate_mask & (1 << self.step)) != 0,
            trigger: reset_edge || clock_edge,
        };
        self.last_trace = Some(self.trace_frame(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: System700SequencerFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_sequencer_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("step"),
            ComponentTraceValue::Float(frame.step as f64),
        )
        .with_output(
            trace_key("cv"),
            ComponentTraceValue::Float(f64::from(frame.cv)),
        )
    }
}

impl Default for System700Sequencer {
    fn default() -> Self {
        Self::new(System700SequencerSettings::default())
    }
}

impl DiscreteComponent for System700Sequencer {
    fn component_id(&self) -> Symbol {
        r700_sequencer_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_sequencer_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_sequencer_params()
    }

    fn reset(&mut self) {
        self.step = 0;
        self.last_clock = false;
        self.last_reset = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(
                input(block.in_audio, 0, frame) > 0.5,
                input(block.in_audio, 1, frame) > 0.5,
            );
            write_outputs(
                block.out_audio,
                frame,
                &[
                    output.cv,
                    if output.gate { 1.0 } else { 0.0 },
                    if output.trigger { 1.0 } else { 0.0 },
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_sequencer_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("step"), self.step.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 sequencer module.
pub fn r700_sequencer_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-sequencer")
}

/// Returns the port descriptors for the System 700 sequencer module.
pub fn r700_sequencer_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("clock-in", ComponentPortMedia::Gate),
        input_port("reset-in", ComponentPortMedia::Gate).optional(),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 sequencer module.
pub fn r700_sequencer_params() -> Vec<ComponentParamDescriptor> {
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

fn sanitize(settings: System700SequencerSettings) -> System700SequencerSettings {
    System700SequencerSettings {
        steps: settings.steps.map(|step| step.clamp(-10.0, 10.0)),
        step_count: settings.step_count.clamp(1, 8),
        gate_mask: settings.gate_mask & 0xff,
    }
}
