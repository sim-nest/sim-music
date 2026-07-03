use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Configuration for a [`System55HighPassFilter`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55HighPassFilterSettings {
    /// Base cutoff frequency in hertz before control-voltage modulation.
    pub cutoff_hz: f32,
    /// Cutoff control-voltage depth, expressed in octaves per volt.
    pub cutoff_cv_depth_octaves: f32,
    /// Pre-filter drive feeding the saturating stages.
    pub drive: f32,
    /// Output level scaling applied after the filter.
    pub level: f32,
}

impl Default for System55HighPassFilterSettings {
    fn default() -> Self {
        Self {
            cutoff_hz: 400.0,
            cutoff_cv_depth_octaves: 1.0,
            drive: 1.0,
            level: 1.0,
        }
    }
}

/// Four-pole saturating high-pass filter with control-voltage cutoff modulation.
#[derive(Clone, Debug, PartialEq)]
pub struct System55HighPassFilter {
    settings: System55HighPassFilterSettings,
    sample_rate_hz: f32,
    stages: [f32; 4],
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55HighPassFilter {
    /// Creates a high-pass filter from `settings`, clamping them into valid ranges.
    pub fn new(settings: System55HighPassFilterSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            stages: [0.0; 4],
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the sanitized settings in effect.
    pub fn settings(&self) -> System55HighPassFilterSettings {
        self.settings
    }

    /// Sets the sample rate in hertz used for coefficient computation.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the cutoff frequency in hertz after applying `cutoff_cv_v` modulation.
    pub fn effective_cutoff_hz(&self, cutoff_cv_v: f32) -> f32 {
        let octaves = cutoff_cv_v * self.settings.cutoff_cv_depth_octaves;
        (self.settings.cutoff_hz * 2.0_f32.powf(octaves)).clamp(5.0, self.nyquist_hz() * 0.98)
    }

    /// Filters one `input` sample under the given cutoff control voltage.
    pub fn next_sample(&mut self, input: f32, cutoff_cv_v: f32) -> f32 {
        let cutoff_hz = self.effective_cutoff_hz(cutoff_cv_v);
        let coefficient = cutoff_coefficient(cutoff_hz, self.sample_rate_hz);
        let mut sample = saturate(input * self.settings.drive);
        for stage in &mut self.stages {
            *stage += coefficient * (sample - *stage);
            sample -= *stage;
        }
        let output = saturate(sample * self.settings.level);
        self.last_trace = Some(self.trace_frame(cutoff_hz, output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn nyquist_hz(&self) -> f32 {
        self.sample_rate_hz * 0.5
    }

    fn trace_frame(&self, cutoff_hz: f32, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            m55_hpf_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("cutoff-hz"),
            ComponentTraceValue::Float(f64::from(cutoff_hz)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System55HighPassFilter {
    fn default() -> Self {
        Self::new(System55HighPassFilterSettings::default())
    }
}

impl DiscreteComponent for System55HighPassFilter {
    fn component_id(&self) -> Symbol {
        m55_hpf_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_hpf_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_hpf_params()
    }

    fn reset(&mut self) {
        self.stages = [0.0; 4];
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

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
        ComponentInspection::new(m55_hpf_component_id(), ComponentBackend::Algorithmic, true)
            .with_field(
                inspect_key("cutoff-hz"),
                self.settings.cutoff_hz.to_string(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 55 high-pass filter module.
pub fn m55_hpf_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-904b-high-pass-filter")
}

/// Returns the port descriptors for the System 55 high-pass filter module.
pub fn m55_hpf_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("cutoff-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 55 high-pass filter module.
pub fn m55_hpf_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("cutoff-hz"), "Cutoff", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(5.0, 20_000.0, 400.0)),
        ComponentParamDescriptor::new(
            param_key("cutoff-cv-depth-octaves"),
            "Cutoff CV depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(param_key("drive"), "Drive", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(0.25, 8.0, 1.0)),
    ]
}

fn sanitize(settings: System55HighPassFilterSettings) -> System55HighPassFilterSettings {
    System55HighPassFilterSettings {
        cutoff_hz: settings.cutoff_hz.clamp(5.0, 20_000.0),
        cutoff_cv_depth_octaves: settings.cutoff_cv_depth_octaves.clamp(0.0, 8.0),
        drive: settings.drive.clamp(0.25, 8.0),
        level: settings.level.clamp(0.0, 2.0),
    }
}

fn cutoff_coefficient(cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
    (1.0 - (-TAU * cutoff_hz / sample_rate_hz).exp()).clamp(0.00005, 0.95)
}

fn saturate(sample: f32) -> f32 {
    sample.tanh().clamp(-1.25, 1.25)
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
