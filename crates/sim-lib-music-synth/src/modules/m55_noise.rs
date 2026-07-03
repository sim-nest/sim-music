use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Spectral color the M55 923 noise source emits as its selected output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System55NoiseColor {
    /// Flat-spectrum white noise.
    White,
    /// Filtered pink noise (falling spectrum).
    Pink,
}

impl System55NoiseColor {
    /// Returns the lowercase identifier string for this noise color.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Pink => "pink",
        }
    }

    /// Returns the qualified `audio-synth/m55-noise-color` symbol for this color.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/m55-noise-color", self.as_str())
    }
}

/// Configuration for the M55 923 noise source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55NoiseSettings {
    /// Spectral color selected for the primary output.
    pub color: System55NoiseColor,
    /// Output level in `[0, 1]`.
    pub level: f32,
    /// Seed for the noise generator's pseudo-random state.
    pub seed: u32,
}

impl Default for System55NoiseSettings {
    fn default() -> Self {
        Self {
            color: System55NoiseColor::White,
            level: 0.6,
            seed: 0x5500_0923,
        }
    }
}

/// One output frame from the noise source, carrying both color taps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55NoiseFrame {
    /// White-noise sample, level-scaled and clamped.
    pub white: f32,
    /// Pink-noise sample, level-scaled and clamped.
    pub pink: f32,
}

/// M55 923 noise source: an xorshift white-noise generator with a one-pole pink
/// filter, exposing both color taps.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Noise {
    settings: System55NoiseSettings,
    rng: u32,
    pink: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Noise {
    /// Builds a noise source from sanitized settings, seeding the generator.
    pub fn new(settings: System55NoiseSettings) -> Self {
        let settings = sanitize(settings);
        Self {
            rng: nonzero_seed(settings.seed),
            settings,
            pink: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the sanitized settings in effect.
    pub fn settings(&self) -> System55NoiseSettings {
        self.settings
    }

    /// Generates the next noise frame (both white and pink taps), records a trace
    /// frame, and advances the clock.
    pub fn next_frame(&mut self) -> System55NoiseFrame {
        let white = self.next_white();
        self.pink = one_pole(self.pink, white, 0.14);
        let frame = System55NoiseFrame {
            white: (white * self.settings.level).clamp(-1.0, 1.0),
            pink: (self.pink * 1.7 * self.settings.level).clamp(-1.0, 1.0),
        };
        self.last_trace = Some(self.trace_frame(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    /// Generates the next sample of the currently selected color.
    pub fn next_sample(&mut self) -> f32 {
        let frame = self.next_frame();
        match self.settings.color {
            System55NoiseColor::White => frame.white,
            System55NoiseColor::Pink => frame.pink,
        }
    }

    fn next_white(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let unit = self.rng as f32 / u32::MAX as f32;
        unit * 2.0 - 1.0
    }

    fn trace_frame(&self, frame: System55NoiseFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            m55_noise_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("color"),
            ComponentTraceValue::Text(self.settings.color.as_str().to_owned()),
        )
        .with_output(
            trace_key("white"),
            ComponentTraceValue::Float(f64::from(frame.white)),
        )
        .with_output(
            trace_key("pink"),
            ComponentTraceValue::Float(f64::from(frame.pink)),
        )
    }
}

impl Default for System55Noise {
    fn default() -> Self {
        Self::new(System55NoiseSettings::default())
    }
}

impl DiscreteComponent for System55Noise {
    fn component_id(&self) -> Symbol {
        m55_noise_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_noise_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_noise_params()
    }

    fn reset(&mut self) {
        self.rng = nonzero_seed(self.settings.seed);
        self.pink = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let output = self.next_frame();
            let selected = match self.settings.color {
                System55NoiseColor::White => output.white,
                System55NoiseColor::Pink => output.pink,
            };
            write_output(block.out_audio, 0, frame, selected);
            write_output(block.out_audio, 1, frame, output.white);
            write_output(block.out_audio, 2, frame, output.pink);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_noise_component_id(),
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

/// Returns the qualified module id for the M55 923 noise source.
pub fn m55_noise_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-923-noise-filter")
}

/// Returns the port descriptors for the M55 923 noise source.
pub fn m55_noise_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("white-out", ComponentPortMedia::AudioRate).optional(),
        output_port("pink-out", ComponentPortMedia::AudioRate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the M55 923 noise source.
pub fn m55_noise_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("color"), "Color", ComponentParamUnit::Unitless)
            .with_enum_values(
                vec![
                    System55NoiseColor::White.symbol(),
                    System55NoiseColor::Pink.symbol(),
                ],
                0,
            ),
        ComponentParamDescriptor::new(param_key("level"), "Level", ComponentParamUnit::Normalized)
            .with_range(ComponentParamRange::new(0.0, 1.0, 0.6)),
        ComponentParamDescriptor::new(param_key("seed"), "Seed", ComponentParamUnit::RawInteger)
            .with_raw_default(i64::from(System55NoiseSettings::default().seed)),
    ]
}

fn sanitize(settings: System55NoiseSettings) -> System55NoiseSettings {
    System55NoiseSettings {
        color: settings.color,
        level: settings.level.clamp(0.0, 1.0),
        seed: nonzero_seed(settings.seed),
    }
}

fn one_pole(state: f32, input: f32, coefficient: f32) -> f32 {
    state + coefficient * (input - state)
}

fn nonzero_seed(seed: u32) -> u32 {
    if seed == 0 { 0x5500_0923 } else { seed }
}

fn write_output(channels: &mut [&mut [f32]], channel: usize, frame: usize, value: f32) {
    if let Some(samples) = channels.get_mut(channel)
        && let Some(sample) = samples.get_mut(frame)
    {
        *sample = value;
    }
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
