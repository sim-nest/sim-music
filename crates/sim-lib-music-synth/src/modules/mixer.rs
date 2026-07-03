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

/// Configuration for a [`System700Mixer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700MixerSettings {
    /// Per-channel input gains.
    pub gains: [f32; 4],
    /// Gain applied to the summed output.
    pub output_gain: f32,
}

impl Default for System700MixerSettings {
    fn default() -> Self {
        Self {
            gains: [1.0; 4],
            output_gain: 1.0,
        }
    }
}

/// Four-channel summing mixer with per-channel and master gains.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Mixer {
    settings: System700MixerSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Mixer {
    /// Creates a mixer from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700MixerSettings) -> Self {
        Self {
            settings: sanitize(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Sums the four `inputs` with their gains and the output gain, clamped to +/-4.
    pub fn mix(&self, inputs: [f32; 4]) -> f32 {
        inputs
            .iter()
            .zip(self.settings.gains)
            .map(|(input, gain)| input * gain)
            .sum::<f32>()
            .mul_add(self.settings.output_gain, 0.0)
            .clamp(-4.0, 4.0)
    }

    /// Mixes one frame of `inputs` and records a trace frame.
    pub fn next_sample(&mut self, inputs: [f32; 4]) -> f32 {
        let output = self.mix(inputs);
        self.last_trace = Some(self.trace_frame(output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn trace_frame(&self, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_mixer_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Mixer {
    fn default() -> Self {
        Self::new(System700MixerSettings::default())
    }
}

impl DiscreteComponent for System700Mixer {
    fn component_id(&self) -> Symbol {
        r700_mixer_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_mixer_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_mixer_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let sample = self.next_sample([
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
                input(block.in_audio, 2, frame),
                input(block.in_audio, 3, frame),
            ]);
            write_outputs(block.out_audio, frame, &[sample]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_mixer_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("channels"), "4")
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 mixer module.
pub fn r700_mixer_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-mixer")
}

/// Returns the port descriptors for the System 700 mixer module.
pub fn r700_mixer_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in-1", ComponentPortMedia::AudioRate),
        input_port("audio-in-2", ComponentPortMedia::AudioRate).optional(),
        input_port("audio-in-3", ComponentPortMedia::AudioRate).optional(),
        input_port("audio-in-4", ComponentPortMedia::AudioRate).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 mixer module.
pub fn r700_mixer_params() -> Vec<ComponentParamDescriptor> {
    vec![
        gain_param("gain-1", "Gain 1", 1.0),
        gain_param("gain-2", "Gain 2", 1.0),
        gain_param("gain-3", "Gain 3", 1.0),
        gain_param("gain-4", "Gain 4", 1.0),
        gain_param("output-gain", "Output gain", 1.0),
    ]
}

fn gain_param(name: &'static str, label: &'static str, default: f64) -> ComponentParamDescriptor {
    ComponentParamDescriptor::new(param_key(name), label, ComponentParamUnit::Unitless)
        .with_range(ComponentParamRange::new(0.0, 2.0, default))
}

fn sanitize(settings: System700MixerSettings) -> System700MixerSettings {
    System700MixerSettings {
        gains: settings.gains.map(|gain| gain.clamp(0.0, 2.0)),
        output_gain: settings.output_gain.clamp(0.0, 2.0),
    }
}
