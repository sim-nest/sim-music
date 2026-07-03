use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Configuration for a [`System55FilterCoupler`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55FilterCouplerSettings {
    /// High-pass cutoff in hertz, setting the lower edge of the pass band.
    pub low_cutoff_hz: f32,
    /// Low-pass cutoff in hertz, setting the upper edge of the pass band.
    pub high_cutoff_hz: f32,
    /// Cutoff control-voltage depth applied to both edges, in octaves per volt.
    pub cutoff_cv_depth_octaves: f32,
    /// Resonance amount emphasizing the low-pass corner.
    pub resonance: f32,
    /// Pre-filter drive feeding the saturating stages.
    pub drive: f32,
    /// Output level scaling applied after the band-pass network.
    pub level: f32,
}

impl Default for System55FilterCouplerSettings {
    fn default() -> Self {
        Self {
            low_cutoff_hz: 250.0,
            high_cutoff_hz: 2_500.0,
            cutoff_cv_depth_octaves: 1.0,
            resonance: 0.2,
            drive: 1.0,
            level: 1.0,
        }
    }
}

/// Band-pass signal coupler chaining a high-pass and low-pass section into one pass band.
#[derive(Clone, Debug, PartialEq)]
pub struct System55FilterCoupler {
    settings: System55FilterCouplerSettings,
    sample_rate_hz: f32,
    high_pass_stages: [f32; 4],
    low_pass_stages: [f32; 4],
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55FilterCoupler {
    /// Creates a coupler from `settings`, clamping them into valid ranges.
    pub fn new(settings: System55FilterCouplerSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            high_pass_stages: [0.0; 4],
            low_pass_stages: [0.0; 4],
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the sanitized settings in effect.
    pub fn settings(&self) -> System55FilterCouplerSettings {
        self.settings
    }

    /// Sets the sample rate in hertz used for coefficient computation.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the `(low, high)` cutoff edges in hertz after applying control-voltage modulation.
    pub fn effective_band_hz(&self, low_cv_v: f32, high_cv_v: f32) -> (f32, f32) {
        let low = self.effective_cutoff(self.settings.low_cutoff_hz, low_cv_v);
        let mut high = self.effective_cutoff(self.settings.high_cutoff_hz, high_cv_v);
        if high <= low {
            high = (low * 1.25).min(self.nyquist_hz() * 0.98);
        }
        (low, high)
    }

    /// Filters one `input` sample through the band defined by the cutoff control voltages.
    pub fn next_sample(&mut self, input: f32, low_cv_v: f32, high_cv_v: f32) -> f32 {
        let (low_cutoff_hz, high_cutoff_hz) = self.effective_band_hz(low_cv_v, high_cv_v);
        let hp_coefficient = cutoff_coefficient(low_cutoff_hz, self.sample_rate_hz);
        let lp_coefficient = cutoff_coefficient(high_cutoff_hz, self.sample_rate_hz);
        let mut sample = saturate(input * self.settings.drive);
        for stage in &mut self.high_pass_stages {
            *stage += hp_coefficient * (sample - *stage);
            sample -= *stage;
        }
        for stage in &mut self.low_pass_stages {
            *stage += lp_coefficient * (sample - *stage);
            sample = *stage;
        }
        let resonant =
            (self.low_pass_stages[2] - self.low_pass_stages[3]) * self.settings.resonance;
        let output = saturate((sample + resonant) * self.settings.level);
        self.last_trace = Some(self.trace_frame(low_cutoff_hz, high_cutoff_hz, output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn effective_cutoff(&self, cutoff_hz: f32, cutoff_cv_v: f32) -> f32 {
        let octaves = cutoff_cv_v * self.settings.cutoff_cv_depth_octaves;
        (cutoff_hz * 2.0_f32.powf(octaves)).clamp(5.0, self.nyquist_hz() * 0.98)
    }

    fn nyquist_hz(&self) -> f32 {
        self.sample_rate_hz * 0.5
    }

    fn trace_frame(
        &self,
        low_cutoff_hz: f32,
        high_cutoff_hz: f32,
        output: f32,
    ) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            m55_coupler_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("low-cutoff-hz"),
            ComponentTraceValue::Float(f64::from(low_cutoff_hz)),
        )
        .with_state(
            trace_key("high-cutoff-hz"),
            ComponentTraceValue::Float(f64::from(high_cutoff_hz)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System55FilterCoupler {
    fn default() -> Self {
        Self::new(System55FilterCouplerSettings::default())
    }
}

impl DiscreteComponent for System55FilterCoupler {
    fn component_id(&self) -> Symbol {
        m55_coupler_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_coupler_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_coupler_params()
    }

    fn reset(&mut self) {
        self.high_pass_stages = [0.0; 4];
        self.low_pass_stages = [0.0; 4];
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
                input(block.in_audio, 2, frame),
            );
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_coupler_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("low-cutoff-hz"),
            self.settings.low_cutoff_hz.to_string(),
        )
        .with_field(
            inspect_key("high-cutoff-hz"),
            self.settings.high_cutoff_hz.to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 55 filter coupler module.
pub fn m55_coupler_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-904c-filter-coupler")
}

/// Returns the port descriptors for the System 55 filter coupler module.
pub fn m55_coupler_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("low-cutoff-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        input_port("high-cutoff-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 55 filter coupler module.
pub fn m55_coupler_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("low-cutoff-hz"),
            "Low cutoff",
            ComponentParamUnit::Hertz,
        )
        .with_range(ComponentParamRange::new(5.0, 10_000.0, 250.0)),
        ComponentParamDescriptor::new(
            param_key("high-cutoff-hz"),
            "High cutoff",
            ComponentParamUnit::Hertz,
        )
        .with_range(ComponentParamRange::new(20.0, 20_000.0, 2_500.0)),
        ComponentParamDescriptor::new(
            param_key("cutoff-cv-depth-octaves"),
            "Cutoff CV depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("resonance"),
            "Resonance",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.2)),
    ]
}

fn sanitize(settings: System55FilterCouplerSettings) -> System55FilterCouplerSettings {
    let low_cutoff_hz = settings.low_cutoff_hz.clamp(5.0, 10_000.0);
    let high_cutoff_hz = settings
        .high_cutoff_hz
        .clamp(low_cutoff_hz * 1.25, 20_000.0);
    System55FilterCouplerSettings {
        low_cutoff_hz,
        high_cutoff_hz,
        cutoff_cv_depth_octaves: settings.cutoff_cv_depth_octaves.clamp(0.0, 8.0),
        resonance: settings.resonance.clamp(0.0, 1.0),
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
