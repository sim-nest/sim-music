use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    modules::vco_driver::System55VcoDriverFrame,
};

const MIN_PULSE_WIDTH: f32 = 0.05;
const MAX_PULSE_WIDTH: f32 = 0.95;

/// Waveform produced by the System 55 voltage-controlled oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System55VcoWaveform {
    /// Rising sawtooth wave.
    Saw,
    /// Triangle wave.
    Triangle,
    /// Rectangular pulse wave with variable width.
    Pulse,
    /// Sine wave.
    Sine,
}

impl System55VcoWaveform {
    /// Returns the lowercase string name of the waveform.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Saw => "saw",
            Self::Triangle => "triangle",
            Self::Pulse => "pulse",
            Self::Sine => "sine",
        }
    }

    /// Returns the qualified symbol identifying this waveform.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/m55-waveform", self.as_str())
    }
}

/// Configuration for the System 55 voltage-controlled oscillator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55VcoSettings {
    /// Output waveform shape.
    pub waveform: System55VcoWaveform,
    /// Base oscillator frequency in hertz at zero pitch CV.
    pub base_frequency_hz: f32,
    /// Pulse width as a fraction of the period, used by the pulse waveform.
    pub pulse_width: f32,
    /// Depth of pulse-width modulation applied by the PWM CV input.
    pub pwm_depth: f32,
    /// Pitch modulation depth in octaves applied by the modulation CV input.
    pub modulation_depth_octaves: f32,
    /// Output level scaling the oscillator signal.
    pub level: f32,
}

impl Default for System55VcoSettings {
    fn default() -> Self {
        Self {
            waveform: System55VcoWaveform::Saw,
            base_frequency_hz: 110.0,
            pulse_width: 0.5,
            pwm_depth: 0.25,
            modulation_depth_octaves: 1.0,
            level: 0.8,
        }
    }
}

/// System 55 voltage-controlled oscillator with pitch, modulation, PWM, and
/// hard-sync inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Vco {
    settings: System55VcoSettings,
    sample_rate_hz: f32,
    phase: f32,
    last_sync_high: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Vco {
    /// Creates an oscillator from sanitized settings at the default sample
    /// rate.
    pub fn new(settings: System55VcoSettings) -> Self {
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
    pub fn settings(&self) -> System55VcoSettings {
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

    /// Computes the oscillator frequency in hertz for the given pitch and
    /// modulation control voltages, clamped to the Nyquist limit.
    pub fn effective_frequency_hz(&self, pitch_cv_v: f32, modulation_cv_v: f32) -> f32 {
        let octaves = pitch_cv_v + modulation_cv_v * self.settings.modulation_depth_octaves;
        (self.settings.base_frequency_hz * 2.0_f32.powf(octaves)).clamp(0.0, self.nyquist_hz())
    }

    /// Computes the effective frequency from a VCO driver frame's pitch and
    /// modulation voltages.
    pub fn effective_frequency_from_driver(&self, frame: System55VcoDriverFrame) -> f32 {
        self.effective_frequency_hz(frame.pitch_cv_v, frame.modulation_cv_v)
    }

    /// Computes the effective pulse width for the given PWM control voltage,
    /// clamped to the supported range.
    pub fn effective_pulse_width(&self, pwm_cv_v: f32) -> f32 {
        (self.settings.pulse_width + pwm_cv_v * self.settings.pwm_depth)
            .clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH)
    }

    /// Advances by one sample, applying hard sync on a rising sync edge, and
    /// returns the oscillator output.
    pub fn next_sample(
        &mut self,
        pitch_cv_v: f32,
        modulation_cv_v: f32,
        pwm_cv_v: f32,
        sync_high: bool,
    ) -> f32 {
        if sync_high && !self.last_sync_high {
            self.phase = 0.0;
        }
        self.last_sync_high = sync_high;

        let frequency_hz = self.effective_frequency_hz(pitch_cv_v, modulation_cv_v);
        let pulse_width = self.effective_pulse_width(pwm_cv_v);
        let sample = self.wave_sample(pulse_width) * self.settings.level;
        self.phase = (self.phase + frequency_hz / self.sample_rate_hz).fract();
        self.last_trace = Some(self.trace_frame(frequency_hz, pulse_width, sample));
        self.clock = self.clock.saturating_add(1);
        sample
    }

    fn wave_sample(&self, pulse_width: f32) -> f32 {
        match self.settings.waveform {
            System55VcoWaveform::Saw => 2.0 * self.phase - 1.0,
            System55VcoWaveform::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            System55VcoWaveform::Pulse => {
                if self.phase < pulse_width {
                    1.0
                } else {
                    -1.0
                }
            }
            System55VcoWaveform::Sine => (TAU * self.phase).sin(),
        }
    }

    fn nyquist_hz(&self) -> f32 {
        self.sample_rate_hz * 0.5
    }

    fn trace_frame(&self, frequency_hz: f32, pulse_width: f32, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            m55_vco_component_id(),
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

impl Default for System55Vco {
    fn default() -> Self {
        Self::new(System55VcoSettings::default())
    }
}

impl DiscreteComponent for System55Vco {
    fn component_id(&self) -> Symbol {
        m55_vco_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_vco_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_vco_params()
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
            let sample = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
                input(block.in_audio, 2, frame),
                input(block.in_audio, 3, frame) > 0.5,
            );
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(m55_vco_component_id(), ComponentBackend::Algorithmic, true)
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

/// Returns the stable component id for the VCO module.
pub fn m55_vco_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-921b-oscillator")
}

/// Returns the port descriptors for the VCO module.
pub fn m55_vco_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("pitch-cv-in", ComponentPortMedia::ControlVoltage),
        input_port("modulation-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        input_port("pwm-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        input_port("sync-in", ComponentPortMedia::Gate).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the VCO module.
pub fn m55_vco_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("waveform"),
            "Waveform",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                System55VcoWaveform::Saw.symbol(),
                System55VcoWaveform::Triangle.symbol(),
                System55VcoWaveform::Pulse.symbol(),
                System55VcoWaveform::Sine.symbol(),
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
            param_key("pwm-depth"),
            "PWM depth",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.25)),
        ComponentParamDescriptor::new(
            param_key("modulation-depth-octaves"),
            "Modulation depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
    ]
}

fn sanitize(settings: System55VcoSettings) -> System55VcoSettings {
    System55VcoSettings {
        waveform: settings.waveform,
        base_frequency_hz: settings.base_frequency_hz.clamp(0.0, 20_000.0),
        pulse_width: settings.pulse_width.clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH),
        pwm_depth: settings.pwm_depth.clamp(0.0, 1.0),
        modulation_depth_octaves: settings.modulation_depth_octaves.clamp(0.0, 8.0),
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
