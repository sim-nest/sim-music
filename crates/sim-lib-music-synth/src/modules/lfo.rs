use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Waveform produced by the low-frequency oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System700LfoWaveform {
    /// Sine wave.
    Sine,
    /// Triangle wave.
    Triangle,
    /// Rising sawtooth wave.
    SawUp,
    /// Falling sawtooth wave.
    SawDown,
    /// Square wave.
    Square,
}

impl System700LfoWaveform {
    /// Returns the lowercase string name of the waveform.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Triangle => "triangle",
            Self::SawUp => "saw-up",
            Self::SawDown => "saw-down",
            Self::Square => "square",
        }
    }

    /// Returns the qualified symbol identifying this waveform.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/r700-lfo-waveform", self.as_str())
    }
}

/// Configuration for the low-frequency oscillator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700LfoSettings {
    /// Output waveform shape.
    pub waveform: System700LfoWaveform,
    /// Base oscillation rate in hertz at zero rate CV.
    pub rate_hz: f32,
    /// Fade-in delay in seconds after the LFO starts.
    pub delay_s: f32,
    /// Depth in octaves of the rate CV input's effect on the rate.
    pub rate_cv_depth_octaves: f32,
    /// Output level scaling the LFO signal.
    pub level: f32,
}

impl Default for System700LfoSettings {
    fn default() -> Self {
        Self {
            waveform: System700LfoWaveform::Sine,
            rate_hz: 5.0,
            delay_s: 0.0,
            rate_cv_depth_octaves: 1.0,
            level: 1.0,
        }
    }
}

/// Low-frequency oscillator with a CV-controllable rate and a fade-in delay.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Lfo {
    settings: System700LfoSettings,
    sample_rate_hz: f32,
    phase: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Lfo {
    /// Creates an LFO from sanitized settings at the default sample rate.
    pub fn new(settings: System700LfoSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the current LFO settings.
    pub fn settings(&self) -> System700LfoSettings {
        self.settings
    }

    /// Sets the processing sample rate in hertz, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the current normalized phase in the range 0 to 1.
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Computes the oscillation rate in hertz for the given rate control
    /// voltage, clamped to the Nyquist limit.
    pub fn effective_rate_hz(&self, rate_cv_v: f32) -> f32 {
        let octaves = rate_cv_v * self.settings.rate_cv_depth_octaves;
        (self.settings.rate_hz * 2.0_f32.powf(octaves)).clamp(0.0, self.sample_rate_hz * 0.5)
    }

    /// Advances by one sample and returns the LFO output, including the
    /// fade-in delay gain.
    pub fn next_sample(&mut self, rate_cv_v: f32) -> f32 {
        let rate = self.effective_rate_hz(rate_cv_v);
        let fade = self.delay_gain();
        let sample = self.wave_sample() * self.settings.level * fade;
        self.phase = (self.phase + rate / self.sample_rate_hz).fract();
        self.last_trace = Some(self.trace_frame(rate, fade, sample));
        self.clock = self.clock.saturating_add(1);
        sample
    }

    fn delay_gain(&self) -> f32 {
        let delay_samples = (self.settings.delay_s * self.sample_rate_hz) as u64;
        if delay_samples == 0 {
            return 1.0;
        }
        (self.clock as f32 / delay_samples as f32).clamp(0.0, 1.0)
    }

    fn wave_sample(&self) -> f32 {
        match self.settings.waveform {
            System700LfoWaveform::Sine => (TAU * self.phase).sin(),
            System700LfoWaveform::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            System700LfoWaveform::SawUp => 2.0 * self.phase - 1.0,
            System700LfoWaveform::SawDown => 1.0 - 2.0 * self.phase,
            System700LfoWaveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }

    fn trace_frame(&self, rate_hz: f32, delay_gain: f32, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_lfo_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("rate-hz"),
            ComponentTraceValue::Float(f64::from(rate_hz)),
        )
        .with_state(
            trace_key("delay-gain"),
            ComponentTraceValue::Float(f64::from(delay_gain)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Lfo {
    fn default() -> Self {
        Self::new(System700LfoSettings::default())
    }
}

impl DiscreteComponent for System700Lfo {
    fn component_id(&self) -> Symbol {
        r700_lfo_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_lfo_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_lfo_params()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let rate_cv = block
                .in_audio
                .first()
                .and_then(|samples| samples.get(frame))
                .copied()
                .unwrap_or(0.0);
            let sample = self.next_sample(rate_cv);
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(r700_lfo_component_id(), ComponentBackend::Algorithmic, true)
            .with_field(
                inspect_key("waveform"),
                self.settings.waveform.as_str().to_owned(),
            )
            .with_field(inspect_key("phase"), self.phase.to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the stable component id for the LFO module.
pub fn r700_lfo_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-lfo")
}

/// Returns the port descriptors for the LFO module.
pub fn r700_lfo_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        ComponentPortDescriptor::new(
            port_key("rate-cv-in"),
            ComponentPortMedia::ControlVoltage,
            ComponentPortDirection::Input,
            1,
        )
        .optional(),
        ComponentPortDescriptor::new(
            port_key("cv-out"),
            ComponentPortMedia::ControlVoltage,
            ComponentPortDirection::Output,
            1,
        ),
        ComponentPortDescriptor::new(
            port_key("trace-out"),
            ComponentPortMedia::Trace,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
    ]
}

/// Returns the parameter descriptors for the LFO module.
pub fn r700_lfo_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("waveform"),
            "Waveform",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                System700LfoWaveform::Sine.symbol(),
                System700LfoWaveform::Triangle.symbol(),
                System700LfoWaveform::SawUp.symbol(),
                System700LfoWaveform::SawDown.symbol(),
                System700LfoWaveform::Square.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(param_key("rate-hz"), "Rate", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(0.01, 100.0, 5.0)),
        ComponentParamDescriptor::new(param_key("delay-s"), "Delay", ComponentParamUnit::Seconds)
            .with_range(ComponentParamRange::new(0.0, 10.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("rate-cv-depth-octaves"),
            "Rate CV depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
    ]
}

fn sanitize(settings: System700LfoSettings) -> System700LfoSettings {
    System700LfoSettings {
        waveform: settings.waveform,
        rate_hz: settings.rate_hz.clamp(0.0, 100.0),
        delay_s: settings.delay_s.clamp(0.0, 10.0),
        rate_cv_depth_octaves: settings.rate_cv_depth_octaves.clamp(0.0, 8.0),
        level: settings.level.clamp(0.0, 1.0),
    }
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
