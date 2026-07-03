use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

const MIN_PULSE_WIDTH: f32 = 0.05;
const MAX_PULSE_WIDTH: f32 = 0.95;

/// Waveform produced by the System 700 voltage-controlled oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System700VcoWaveform {
    /// Rising sawtooth.
    Saw,
    /// Triangle wave.
    Triangle,
    /// Pulse wave with variable width.
    Pulse,
    /// Sine wave.
    Sine,
}

impl System700VcoWaveform {
    /// Returns the stable string name of this waveform.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Saw => "saw",
            Self::Triangle => "triangle",
            Self::Pulse => "pulse",
            Self::Sine => "sine",
        }
    }

    /// Returns the qualified [`Symbol`] naming this waveform.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/r700-waveform", self.as_str())
    }
}

/// Settings for the System 700 voltage-controlled oscillator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700VcoSettings {
    /// Output waveform.
    pub waveform: System700VcoWaveform,
    /// Base oscillator frequency at zero pitch CV, in hertz.
    pub base_frequency_hz: f32,
    /// Pulse width for the pulse waveform, normalized.
    pub pulse_width: f32,
    /// Depth of pulse-width modulation applied per unit of PWM CV.
    pub pwm_depth: f32,
    /// Octaves of exponential FM applied per volt of FM CV.
    pub exp_fm_depth_octaves: f32,
    /// Output level.
    pub level: f32,
}

impl Default for System700VcoSettings {
    fn default() -> Self {
        Self {
            waveform: System700VcoWaveform::Saw,
            base_frequency_hz: 110.0,
            pulse_width: 0.5,
            pwm_depth: 0.25,
            exp_fm_depth_octaves: 1.0,
            level: 0.8,
        }
    }
}

/// The System 700 voltage-controlled oscillator module: a phase accumulator
/// driving a selectable waveform with pitch CV, exponential FM, PWM, and sync.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Vco {
    settings: System700VcoSettings,
    sample_rate_hz: f32,
    phase: f32,
    last_sync_high: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Vco {
    /// Creates an oscillator with sanitized `settings` at the default sample rate.
    pub fn new(settings: System700VcoSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            phase: 0.0,
            last_sync_high: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the current oscillator settings.
    pub fn settings(&self) -> System700VcoSettings {
        self.settings
    }

    /// Sets the working sample rate, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the current phase accumulator value in `0.0..1.0`.
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Returns the oscillator frequency for the given pitch CV and exponential
    /// FM CV, clamped to the Nyquist limit.
    pub fn effective_frequency_hz(&self, pitch_cv_v: f32, exp_fm_cv: f32) -> f32 {
        let octaves = pitch_cv_v + exp_fm_cv * self.settings.exp_fm_depth_octaves;
        (self.settings.base_frequency_hz * 2.0_f32.powf(octaves)).clamp(0.0, self.nyquist_hz())
    }

    /// Returns the pulse width for the given PWM CV, clamped to the legal range.
    pub fn effective_pulse_width(&self, pwm_cv: f32) -> f32 {
        (self.settings.pulse_width + pwm_cv * self.settings.pwm_depth)
            .clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH)
    }

    /// Advances the oscillator one sample, applying a hard reset on a rising
    /// `sync_high` edge, and returns the leveled output sample.
    pub fn next_sample(
        &mut self,
        pitch_cv_v: f32,
        exp_fm_cv: f32,
        pwm_cv: f32,
        sync_high: bool,
    ) -> f32 {
        if sync_high && !self.last_sync_high {
            self.phase = 0.0;
        }
        self.last_sync_high = sync_high;

        let frequency = self.effective_frequency_hz(pitch_cv_v, exp_fm_cv);
        let pulse_width = self.effective_pulse_width(pwm_cv);
        let sample = self.wave_sample(pulse_width) * self.settings.level;
        let delta = frequency / self.sample_rate_hz;
        self.phase = (self.phase + delta).fract();
        self.last_trace = Some(self.trace_frame(frequency, pulse_width, sample));
        self.clock = self.clock.saturating_add(1);
        sample
    }

    fn wave_sample(&self, pulse_width: f32) -> f32 {
        match self.settings.waveform {
            System700VcoWaveform::Saw => 2.0 * self.phase - 1.0,
            System700VcoWaveform::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            System700VcoWaveform::Pulse => {
                if self.phase < pulse_width {
                    1.0
                } else {
                    -1.0
                }
            }
            System700VcoWaveform::Sine => (TAU * self.phase).sin(),
        }
    }

    fn nyquist_hz(&self) -> f32 {
        self.sample_rate_hz * 0.5
    }

    fn trace_frame(&self, frequency_hz: f32, pulse_width: f32, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_vco_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("frequency-hz"),
            ComponentTraceValue::Float(f64::from(frequency_hz)),
        )
        .with_state(
            trace_key("pulse-width"),
            ComponentTraceValue::Float(f64::from(pulse_width)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Vco {
    fn default() -> Self {
        Self::new(System700VcoSettings::default())
    }
}

impl DiscreteComponent for System700Vco {
    fn component_id(&self) -> Symbol {
        r700_vco_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_vco_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_vco_params()
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.last_sync_high = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let pitch = input(block.in_audio, 0, frame);
            let fm = input(block.in_audio, 1, frame);
            let pwm = input(block.in_audio, 2, frame);
            let sync = input(block.in_audio, 3, frame) > 0.5;
            let sample = self.next_sample(pitch, fm, pwm, sync);
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(r700_vco_component_id(), ComponentBackend::Algorithmic, true)
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

/// Returns the component id symbol for the System 700 VCO module.
pub fn r700_vco_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-vco")
}

/// Returns the port descriptors for the System 700 VCO module.
pub fn r700_vco_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("pitch-cv-in", ComponentPortMedia::ControlVoltage),
        input_port("exp-fm-in", ComponentPortMedia::ControlVoltage).optional(),
        input_port("pwm-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        input_port("sync-in", ComponentPortMedia::Gate).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 VCO module.
pub fn r700_vco_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("waveform"),
            "Waveform",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                System700VcoWaveform::Saw.symbol(),
                System700VcoWaveform::Triangle.symbol(),
                System700VcoWaveform::Pulse.symbol(),
                System700VcoWaveform::Sine.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(
            param_key("base-frequency-hz"),
            "Base frequency",
            ComponentParamUnit::Hertz,
        )
        .with_range(ComponentParamRange::new(8.0, 16_000.0, 110.0)),
        ComponentParamDescriptor::new(
            param_key("pulse-width"),
            "Pulse width",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.05, 0.95, 0.5)),
        ComponentParamDescriptor::new(
            param_key("exp-fm-depth-octaves"),
            "Exponential FM depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
    ]
}

fn sanitize(settings: System700VcoSettings) -> System700VcoSettings {
    System700VcoSettings {
        waveform: settings.waveform,
        base_frequency_hz: settings.base_frequency_hz.clamp(0.0, 20_000.0),
        pulse_width: settings.pulse_width.clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH),
        pwm_depth: settings.pwm_depth.clamp(0.0, 1.0),
        exp_fm_depth_octaves: settings.exp_fm_depth_octaves.clamp(0.0, 8.0),
        level: settings.level.clamp(0.0, 1.0),
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
