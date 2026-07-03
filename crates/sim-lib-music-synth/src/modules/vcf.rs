use std::f32::consts::PI;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Filter response selected on the System 700 voltage-controlled filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System700VcfMode {
    /// Low-pass response.
    LowPass,
    /// Band-pass response.
    BandPass,
    /// High-pass response.
    HighPass,
    /// Notch (band-reject) response.
    Notch,
}

impl System700VcfMode {
    /// Returns the stable string name of this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LowPass => "lowpass",
            Self::BandPass => "bandpass",
            Self::HighPass => "highpass",
            Self::Notch => "notch",
        }
    }

    /// Returns the qualified [`Symbol`] naming this mode.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/r700-vcf-mode", self.as_str())
    }
}

/// Settings for the System 700 voltage-controlled filter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700VcfSettings {
    /// Filter response mode.
    pub mode: System700VcfMode,
    /// Base cutoff frequency at zero cutoff CV, in hertz.
    pub cutoff_hz: f32,
    /// Resonance amount; values above 1.0 drive self-oscillation.
    pub resonance: f32,
    /// Octaves of cutoff sweep applied per volt of cutoff CV.
    pub cutoff_cv_depth_octaves: f32,
    /// Output level.
    pub level: f32,
}

impl Default for System700VcfSettings {
    fn default() -> Self {
        Self {
            mode: System700VcfMode::LowPass,
            cutoff_hz: 1_000.0,
            resonance: 0.2,
            cutoff_cv_depth_octaves: 1.0,
            level: 1.0,
        }
    }
}

/// The System 700 voltage-controlled filter module: a state-variable filter
/// with CV-swept cutoff, resonance, selectable response, and self-oscillation.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Vcf {
    settings: System700VcfSettings,
    sample_rate_hz: f32,
    low: f32,
    band: f32,
    osc_phase: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Vcf {
    /// Creates a filter with sanitized `settings` at the default sample rate.
    pub fn new(settings: System700VcfSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            low: 0.0,
            band: 0.0,
            osc_phase: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the current filter settings.
    pub fn settings(&self) -> System700VcfSettings {
        self.settings
    }

    /// Sets the working sample rate, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the cutoff frequency for the given cutoff CV, swept by
    /// `cutoff_cv_depth_octaves` and clamped below Nyquist.
    pub fn effective_cutoff_hz(&self, cutoff_cv_v: f32) -> f32 {
        let octaves = cutoff_cv_v * self.settings.cutoff_cv_depth_octaves;
        (self.settings.cutoff_hz * 2.0_f32.powf(octaves)).clamp(5.0, self.nyquist_hz() * 0.98)
    }

    /// Returns the configured resonance amount.
    pub fn resonance(&self) -> f32 {
        self.settings.resonance
    }

    /// Processes one input sample with cutoff CV `cutoff_cv_v` and returns the
    /// leveled filter output for the selected mode.
    pub fn next_sample(&mut self, input: f32, cutoff_cv_v: f32) -> f32 {
        let cutoff = self.effective_cutoff_hz(cutoff_cv_v);
        let f = (2.0 * (PI * cutoff / self.sample_rate_hz).sin()).clamp(0.0001, 0.99);
        let damping = (2.0 - self.settings.resonance * 1.6).clamp(0.05, 2.0);

        self.low += f * self.band;
        let high = input.clamp(-4.0, 4.0) - self.low - damping * self.band;
        self.band += f * high;
        let notch = high + self.low;
        let filtered = match self.settings.mode {
            System700VcfMode::LowPass => self.low,
            System700VcfMode::BandPass => self.band,
            System700VcfMode::HighPass => high,
            System700VcfMode::Notch => notch,
        };
        let self_oscillation = self.self_oscillation(cutoff);
        let output = ((filtered + self_oscillation) * self.settings.level).clamp(-4.0, 4.0);
        self.last_trace = Some(self.trace_frame(cutoff, output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn self_oscillation(&mut self, cutoff_hz: f32) -> f32 {
        if self.settings.resonance <= 1.0 {
            return 0.0;
        }
        let amount = ((self.settings.resonance - 1.0) / 0.25).clamp(0.0, 1.0) * 0.35;
        let sample = (self.osc_phase * std::f32::consts::TAU).sin() * amount;
        self.osc_phase = (self.osc_phase + cutoff_hz / self.sample_rate_hz).fract();
        sample
    }

    fn nyquist_hz(&self) -> f32 {
        self.sample_rate_hz * 0.5
    }

    fn trace_frame(&self, cutoff_hz: f32, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_vcf_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("cutoff-hz"),
            ComponentTraceValue::Float(f64::from(cutoff_hz)),
        )
        .with_state(
            trace_key("resonance"),
            ComponentTraceValue::Float(f64::from(self.settings.resonance)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Vcf {
    fn default() -> Self {
        Self::new(System700VcfSettings::default())
    }
}

impl DiscreteComponent for System700Vcf {
    fn component_id(&self) -> Symbol {
        r700_vcf_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_vcf_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_vcf_params()
    }

    fn reset(&mut self) {
        self.low = 0.0;
        self.band = 0.0;
        self.osc_phase = 0.0;
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
        ComponentInspection::new(r700_vcf_component_id(), ComponentBackend::Algorithmic, true)
            .with_field(inspect_key("mode"), self.settings.mode.as_str().to_owned())
            .with_field(
                inspect_key("cutoff-hz"),
                self.settings.cutoff_hz.to_string(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id symbol for the System 700 VCF module.
pub fn r700_vcf_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-vcf")
}

/// Returns the qualified symbols for every [`System700VcfMode`], in declaration
/// order.
pub fn r700_vcf_mode_symbols() -> [Symbol; 4] {
    [
        System700VcfMode::LowPass.symbol(),
        System700VcfMode::BandPass.symbol(),
        System700VcfMode::HighPass.symbol(),
        System700VcfMode::Notch.symbol(),
    ]
}

/// Returns the port descriptors for the System 700 VCF module.
pub fn r700_vcf_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("cutoff-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 VCF module.
pub fn r700_vcf_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("mode"), "Mode", ComponentParamUnit::Unitless)
            .with_enum_values(r700_vcf_mode_symbols().to_vec(), 0),
        ComponentParamDescriptor::new(param_key("cutoff-hz"), "Cutoff", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(5.0, 20_000.0, 1_000.0)),
        ComponentParamDescriptor::new(
            param_key("resonance"),
            "Resonance",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 1.25, 0.2)),
        ComponentParamDescriptor::new(
            param_key("cutoff-cv-depth-octaves"),
            "Cutoff CV depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
    ]
}

fn sanitize(settings: System700VcfSettings) -> System700VcfSettings {
    System700VcfSettings {
        mode: settings.mode,
        cutoff_hz: settings.cutoff_hz.clamp(5.0, 20_000.0),
        resonance: settings.resonance.clamp(0.0, 1.25),
        cutoff_cv_depth_octaves: settings.cutoff_cv_depth_octaves.clamp(0.0, 8.0),
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
