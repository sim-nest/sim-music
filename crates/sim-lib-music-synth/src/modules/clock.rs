use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use super::common::{input, input_port, output_port, param_key, trace_key, write_outputs};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortMedia, ComponentPrepareConfig,
    ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Configuration for a [`System700Clock`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700ClockSettings {
    /// Clock rate in hertz (pulses per second).
    pub rate_hz: f32,
    /// Fraction of each cycle the gate stays high, in `0.0..=1.0`.
    pub pulse_width: f32,
}

impl Default for System700ClockSettings {
    fn default() -> Self {
        Self {
            rate_hz: 2.0,
            pulse_width: 0.5,
        }
    }
}

/// Outputs produced for one clock frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System700ClockFrame {
    /// Whether the gate is currently high.
    pub gate: bool,
    /// Whether the gate rose on this frame.
    pub trigger: bool,
    /// Phase within the current cycle, in `0.0..1.0`.
    pub phase: f32,
}

/// Free-running clock generator producing gate, trigger, and phase outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Clock {
    settings: System700ClockSettings,
    sample_rate_hz: f32,
    phase: f32,
    last_gate: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Clock {
    /// Creates a clock from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700ClockSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            last_gate: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the sample rate in hertz used to advance the phase.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Advances the clock by one sample, gated by `run`, and returns the frame.
    pub fn next_frame(&mut self, run: bool) -> System700ClockFrame {
        let gate = run && self.phase < self.settings.pulse_width;
        let frame = System700ClockFrame {
            gate,
            trigger: gate && !self.last_gate,
            phase: self.phase,
        };
        self.last_gate = gate;
        if run {
            self.phase = (self.phase + self.settings.rate_hz / self.sample_rate_hz).fract();
        }
        self.last_trace = Some(self.trace_frame(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: System700ClockFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_clock_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(trace_key("gate"), ComponentTraceValue::Bool(frame.gate))
        .with_output(
            trace_key("phase"),
            ComponentTraceValue::Float(f64::from(frame.phase)),
        )
    }
}

impl Default for System700Clock {
    fn default() -> Self {
        Self::new(System700ClockSettings::default())
    }
}

impl DiscreteComponent for System700Clock {
    fn component_id(&self) -> Symbol {
        r700_clock_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_clock_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_clock_params()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.last_gate = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let run = block.in_audio.is_empty() || input(block.in_audio, 0, frame) > 0.5;
            let output = self.next_frame(run);
            write_outputs(
                block.out_audio,
                frame,
                &[
                    if output.gate { 1.0 } else { 0.0 },
                    if output.trigger { 1.0 } else { 0.0 },
                    output.phase,
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_clock_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 clock module.
pub fn r700_clock_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-clock")
}

/// Returns the port descriptors for the System 700 clock module.
pub fn r700_clock_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("run-in", ComponentPortMedia::Gate).optional(),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("trigger-out", ComponentPortMedia::Gate),
        output_port("phase-out", ComponentPortMedia::ControlRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 clock module.
pub fn r700_clock_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("rate-hz"), "Rate", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(0.01, 200.0, 2.0)),
        ComponentParamDescriptor::new(
            param_key("pulse-width"),
            "Pulse width",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.01, 0.99, 0.5)),
    ]
}

fn sanitize(settings: System700ClockSettings) -> System700ClockSettings {
    System700ClockSettings {
        rate_hz: settings.rate_hz.clamp(0.01, 200.0),
        pulse_width: settings.pulse_width.clamp(0.01, 0.99),
    }
}
