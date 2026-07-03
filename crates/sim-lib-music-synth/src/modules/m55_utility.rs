use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Configuration for the System 55 four-channel mixer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55MixerSettings {
    /// Per-channel input gains.
    pub gains: [f32; 4],
    /// Overall gain applied to the summed mix.
    pub output_gain: f32,
    /// Saturation drive applied through the tanh shaper.
    pub drive: f32,
}

impl Default for System55MixerSettings {
    fn default() -> Self {
        Self {
            gains: [1.0; 4],
            output_gain: 1.0,
            drive: 1.5,
        }
    }
}

/// System 55 four-channel mixer with per-channel gains and a saturating
/// output stage.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Mixer {
    settings: System55MixerSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Mixer {
    /// Creates a mixer from sanitized settings.
    pub fn new(settings: System55MixerSettings) -> Self {
        Self {
            settings: sanitize_mixer(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Mixes four inputs through their gains and the saturating output stage,
    /// returning the clamped result.
    pub fn mix(&self, inputs: [f32; 4]) -> f32 {
        let sum = inputs
            .iter()
            .zip(self.settings.gains)
            .map(|(input, gain)| input * gain)
            .sum::<f32>()
            * self.settings.output_gain;
        let drive = self.settings.drive.max(0.001);
        ((sum * drive).tanh() / drive.tanh()).clamp(-4.0, 4.0)
    }

    /// Advances by one sample and returns the mixed output.
    pub fn next_sample(&mut self, inputs: [f32; 4]) -> f32 {
        let output = self.mix(inputs);
        self.last_trace = Some(trace_output(m55_mixer_component_id(), self.clock, output));
        self.clock = self.clock.saturating_add(1);
        output
    }
}

impl Default for System55Mixer {
    fn default() -> Self {
        Self::new(System55MixerSettings::default())
    }
}

impl DiscreteComponent for System55Mixer {
    fn component_id(&self) -> Symbol {
        m55_mixer_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_mixer_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_mixer_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_sample([
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
                input(block.in_audio, 2, frame),
                input(block.in_audio, 3, frame),
            ]);
            write_outputs(block.out_audio, frame, &[output]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_mixer_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("channels"), "4")
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for the System 55 multiple (signal splitter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55MultipleSettings {
    /// Number of active output copies, from 1 to 4.
    pub output_count: usize,
}

impl Default for System55MultipleSettings {
    fn default() -> Self {
        Self { output_count: 4 }
    }
}

/// Per-sample output of the System 55 multiple.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55MultipleFrame {
    /// Output copies; channels beyond `output_count` are left at zero.
    pub outputs: [f32; 4],
}

/// System 55 multiple that fans one input out to up to four identical copies.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Multiple {
    settings: System55MultipleSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Multiple {
    /// Creates a multiple with the output count clamped to 1 through 4.
    pub fn new(settings: System55MultipleSettings) -> Self {
        Self {
            settings: System55MultipleSettings {
                output_count: settings.output_count.clamp(1, 4),
            },
            clock: 0,
            last_trace: None,
        }
    }

    /// Advances by one sample, copying `input` to each active output.
    pub fn next_frame(&mut self, input: f32) -> System55MultipleFrame {
        let mut outputs = [0.0; 4];
        outputs
            .iter_mut()
            .take(self.settings.output_count)
            .for_each(|output| *output = input);
        self.last_trace = Some(trace_output(m55_multiple_component_id(), self.clock, input));
        self.clock = self.clock.saturating_add(1);
        System55MultipleFrame { outputs }
    }
}

impl Default for System55Multiple {
    fn default() -> Self {
        Self::new(System55MultipleSettings::default())
    }
}

impl DiscreteComponent for System55Multiple {
    fn component_id(&self) -> Symbol {
        m55_multiple_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_multiple_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_multiple_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &output.outputs);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_multiple_component_id(),
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

/// Configuration for the System 55 attenuator/offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55AttenuatorSettings {
    /// Scaling gain applied to the input.
    pub gain: f32,
    /// Constant offset added after scaling.
    pub offset: f32,
}

impl Default for System55AttenuatorSettings {
    fn default() -> Self {
        Self {
            gain: 1.0,
            offset: 0.0,
        }
    }
}

/// System 55 attenuator that scales and offsets a control voltage.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Attenuator {
    settings: System55AttenuatorSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Attenuator {
    /// Creates an attenuator from sanitized settings.
    pub fn new(settings: System55AttenuatorSettings) -> Self {
        Self {
            settings: sanitize_attenuator(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Advances by one sample, returning the scaled, offset, clamped output.
    pub fn next_sample(&mut self, input: f32) -> f32 {
        let output = (input * self.settings.gain + self.settings.offset).clamp(-10.0, 10.0);
        self.last_trace = Some(trace_output(
            m55_attenuator_component_id(),
            self.clock,
            output,
        ));
        self.clock = self.clock.saturating_add(1);
        output
    }
}

impl Default for System55Attenuator {
    fn default() -> Self {
        Self::new(System55AttenuatorSettings::default())
    }
}

impl DiscreteComponent for System55Attenuator {
    fn component_id(&self) -> Symbol {
        m55_attenuator_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_attenuator_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_attenuator_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_sample(input(block.in_audio, 0, frame));
            write_outputs(block.out_audio, frame, &[output]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_attenuator_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("gain"), self.settings.gain.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the stable component id for the mixer module.
pub fn m55_mixer_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-cp3a-mixer")
}

/// Returns the stable component id for the multiple module.
pub fn m55_multiple_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-multiple")
}

/// Returns the stable component id for the attenuator module.
pub fn m55_attenuator_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-attenuator")
}

/// Returns the port descriptors for the mixer module.
pub fn m55_mixer_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in-1", ComponentPortMedia::AudioRate),
        input_port("audio-in-2", ComponentPortMedia::AudioRate).optional(),
        input_port("audio-in-3", ComponentPortMedia::AudioRate).optional(),
        input_port("audio-in-4", ComponentPortMedia::AudioRate).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the port descriptors for the multiple module.
pub fn m55_multiple_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("signal-in", ComponentPortMedia::ControlVoltage),
        output_port("signal-out-1", ComponentPortMedia::ControlVoltage),
        output_port("signal-out-2", ComponentPortMedia::ControlVoltage),
        output_port("signal-out-3", ComponentPortMedia::ControlVoltage),
        output_port("signal-out-4", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the port descriptors for the attenuator module.
pub fn m55_attenuator_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("signal-in", ComponentPortMedia::ControlVoltage),
        output_port("signal-out", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the mixer module.
pub fn m55_mixer_params() -> Vec<ComponentParamDescriptor> {
    vec![
        gain_param("gain-1", "Gain 1", 1.0),
        gain_param("gain-2", "Gain 2", 1.0),
        gain_param("gain-3", "Gain 3", 1.0),
        gain_param("gain-4", "Gain 4", 1.0),
        gain_param("output-gain", "Output gain", 1.0),
        ComponentParamDescriptor::new(param_key("drive"), "Drive", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(0.1, 8.0, 1.5)),
    ]
}

/// Returns the parameter descriptors for the multiple module.
pub fn m55_multiple_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("output-count"),
            "Output count",
            ComponentParamUnit::RawInteger,
        )
        .with_range(ComponentParamRange::new(1.0, 4.0, 4.0))
        .with_raw_default(4),
    ]
}

/// Returns the parameter descriptors for the attenuator module.
pub fn m55_attenuator_params() -> Vec<ComponentParamDescriptor> {
    vec![
        gain_param("gain", "Gain", 1.0),
        ComponentParamDescriptor::new(param_key("offset"), "Offset", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(-10.0, 10.0, 0.0)),
    ]
}

fn sanitize_mixer(settings: System55MixerSettings) -> System55MixerSettings {
    System55MixerSettings {
        gains: settings.gains.map(|gain| gain.clamp(0.0, 2.0)),
        output_gain: settings.output_gain.clamp(0.0, 2.0),
        drive: settings.drive.clamp(0.1, 8.0),
    }
}

fn sanitize_attenuator(settings: System55AttenuatorSettings) -> System55AttenuatorSettings {
    System55AttenuatorSettings {
        gain: settings.gain.clamp(-2.0, 2.0),
        offset: settings.offset.clamp(-10.0, 10.0),
    }
}

fn gain_param(name: &'static str, label: &'static str, default: f64) -> ComponentParamDescriptor {
    ComponentParamDescriptor::new(param_key(name), label, ComponentParamUnit::Unitless)
        .with_range(ComponentParamRange::new(0.0, 2.0, default))
}

fn trace_output(id: Symbol, clock: u64, output: f32) -> ComponentTraceFrame {
    ComponentTraceFrame::new(id, ComponentBackend::Algorithmic, clock).with_output(
        trace_key("output"),
        ComponentTraceValue::Float(f64::from(output)),
    )
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
