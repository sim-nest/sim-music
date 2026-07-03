//! Deterministic System 700 noise sources.
//!
//! White noise uses a local xorshift32 generator. Colored outputs pass the same
//! synthetic stream through one-pole smoothing states so tests and fixtures stay
//! reproducible without third-party samples.

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Spectral color of the noise produced by a [`System700Noise`] source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System700NoiseColor {
    /// Flat-spectrum white noise straight from the generator.
    White,
    /// Pink noise: white smoothed by a one-pole filter for a -3 dB/octave tilt.
    Pink,
    /// Red (Brownian) noise: white heavily smoothed toward the low end.
    Red,
}

impl System700NoiseColor {
    /// Returns the stable lowercase identifier for this noise color.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Pink => "pink",
            Self::Red => "red",
        }
    }

    /// Returns the qualified [`Symbol`] naming this noise color.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/r700-noise-color", self.as_str())
    }
}

/// Configuration for a [`System700Noise`] source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700NoiseSettings {
    /// Spectral color of the output.
    pub color: System700NoiseColor,
    /// Output level scaling in `0.0..=1.0`.
    pub level: f32,
    /// Seed for the deterministic xorshift32 generator.
    pub seed: u32,
}

impl Default for System700NoiseSettings {
    fn default() -> Self {
        Self {
            color: System700NoiseColor::White,
            level: 0.6,
            seed: 0x7000_0001,
        }
    }
}

/// Deterministic white, pink, or red noise source built on a xorshift32 generator.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Noise {
    settings: System700NoiseSettings,
    rng: u32,
    pink: f32,
    red: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Noise {
    /// Creates a noise source from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700NoiseSettings) -> Self {
        let settings = sanitize(settings);
        Self {
            rng: nonzero_seed(settings.seed),
            settings,
            pink: 0.0,
            red: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the sanitized settings in effect.
    pub fn settings(&self) -> System700NoiseSettings {
        self.settings
    }

    /// Advances the generator and returns the next colored noise sample.
    pub fn next_sample(&mut self) -> f32 {
        let white = self.next_white();
        let colored = match self.settings.color {
            System700NoiseColor::White => white,
            System700NoiseColor::Pink => {
                self.pink = one_pole(self.pink, white, 0.14);
                self.pink * 1.7
            }
            System700NoiseColor::Red => {
                self.red = one_pole(self.red, white, 0.025);
                self.red * 3.0
            }
        };
        let sample = (colored * self.settings.level).clamp(-1.0, 1.0);
        self.last_trace = Some(self.trace_frame(sample));
        self.clock = self.clock.saturating_add(1);
        sample
    }

    fn next_white(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let unit = self.rng as f32 / u32::MAX as f32;
        unit * 2.0 - 1.0
    }

    fn trace_frame(&self, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_noise_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("color"),
            ComponentTraceValue::Text(self.settings.color.as_str().to_owned()),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Noise {
    fn default() -> Self {
        Self::new(System700NoiseSettings::default())
    }
}

impl DiscreteComponent for System700Noise {
    fn component_id(&self) -> Symbol {
        r700_noise_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_noise_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_noise_params()
    }

    fn reset(&mut self) {
        self.rng = nonzero_seed(self.settings.seed);
        self.pink = 0.0;
        self.red = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let sample = self.next_sample();
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_noise_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("color"),
            self.settings.color.as_str().to_owned(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 noise module.
pub fn r700_noise_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-noise")
}

/// Returns the port descriptors for the System 700 noise module.
pub fn r700_noise_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        ComponentPortDescriptor::new(
            port_key("audio-out"),
            ComponentPortMedia::AudioRate,
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

/// Returns the parameter descriptors for the System 700 noise module.
pub fn r700_noise_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("color"), "Color", ComponentParamUnit::Unitless)
            .with_enum_values(
                vec![
                    System700NoiseColor::White.symbol(),
                    System700NoiseColor::Pink.symbol(),
                    System700NoiseColor::Red.symbol(),
                ],
                0,
            ),
        ComponentParamDescriptor::new(param_key("level"), "Level", ComponentParamUnit::Normalized)
            .with_range(ComponentParamRange::new(0.0, 1.0, 0.6)),
        ComponentParamDescriptor::new(param_key("seed"), "Seed", ComponentParamUnit::RawInteger)
            .with_raw_default(i64::from(System700NoiseSettings::default().seed)),
    ]
}

fn sanitize(settings: System700NoiseSettings) -> System700NoiseSettings {
    System700NoiseSettings {
        color: settings.color,
        level: settings.level.clamp(0.0, 1.0),
        seed: nonzero_seed(settings.seed),
    }
}

fn one_pole(state: f32, input: f32, coefficient: f32) -> f32 {
    state + coefficient * (input - state)
}

fn nonzero_seed(seed: u32) -> u32 {
    if seed == 0 { 0x7000_0001 } else { seed }
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
