use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Configuration for a [`System700RingModulator`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700RingSettings {
    /// Output level scaling applied to the modulated product.
    pub level: f32,
}

impl Default for System700RingSettings {
    fn default() -> Self {
        Self { level: 1.0 }
    }
}

/// Ring modulator that multiplies a carrier and modulator signal.
#[derive(Clone, Debug, PartialEq)]
pub struct System700RingModulator {
    settings: System700RingSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700RingModulator {
    /// Creates a ring modulator from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700RingSettings) -> Self {
        Self {
            settings: sanitize(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the sanitized settings in effect.
    pub fn settings(&self) -> System700RingSettings {
        self.settings
    }

    /// Multiplies `carrier` by `modulator`, scales by level, and records a trace.
    pub fn next_sample(&mut self, carrier: f32, modulator: f32) -> f32 {
        let output = (carrier * modulator * self.settings.level).clamp(-2.0, 2.0);
        self.last_trace = Some(self.trace_frame(output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn trace_frame(&self, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_ring_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700RingModulator {
    fn default() -> Self {
        Self::new(System700RingSettings::default())
    }
}

impl DiscreteComponent for System700RingModulator {
    fn component_id(&self) -> Symbol {
        r700_ring_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_ring_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_ring_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let sample = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_ring_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("level"), self.settings.level.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 ring modulator module.
pub fn r700_ring_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-ring")
}

/// Returns the port descriptors for the System 700 ring modulator module.
pub fn r700_ring_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("carrier-in", ComponentPortMedia::AudioRate),
        input_port("modulator-in", ComponentPortMedia::AudioRate),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 ring modulator module.
pub fn r700_ring_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("level"), "Level", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(0.0, 2.0, 1.0)),
    ]
}

fn sanitize(settings: System700RingSettings) -> System700RingSettings {
    System700RingSettings {
        level: settings.level.clamp(0.0, 2.0),
    }
}

fn input(channels: &[&[f32]], channel: usize, frame: usize) -> f32 {
    channels
        .get(channel)
        .and_then(|samples| samples.get(frame))
        .copied()
        .unwrap_or(0.0)
}

fn input_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Input, 1)
}

fn output_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Output, 1)
}

fn port_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-port", name)
}

fn param_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-param", name)
}

fn inspect_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-inspect", name)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-trace", name)
}
