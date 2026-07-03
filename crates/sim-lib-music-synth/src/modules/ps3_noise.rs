use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Selects which spectral flavor of the PS-3300 noise source is output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300NoiseColor {
    /// Full-spectrum white noise.
    White,
    /// One-pole low-pass "colored" noise.
    Colored,
}

impl Ps3300NoiseColor {
    /// Returns the stable kebab-case identifier string for this color.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Colored => "colored",
        }
    }

    /// Returns the qualified `audio-synth/ps3300-noise-color` symbol for this color.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-noise-color", self.as_str())
    }
}

/// Configuration for the PS-3300 noise source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300NoiseSettings {
    /// Which noise color is selected for the main output.
    pub color: Ps3300NoiseColor,
    /// Output level (0.0 to 1.0) applied to the noise.
    pub level: f32,
    /// Seed for the deterministic xorshift random generator.
    pub seed: u32,
    /// One-pole coefficient shaping the colored-noise low-pass filter.
    pub color_coefficient: f32,
}

impl Default for Ps3300NoiseSettings {
    fn default() -> Self {
        Self {
            color: Ps3300NoiseColor::White,
            level: 0.6,
            seed: 0x3300_0001,
            color_coefficient: 0.08,
        }
    }
}

/// One rendered sample from the noise source, exposing both colors and the selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300NoiseFrame {
    /// Leveled white-noise sample.
    pub white: f32,
    /// Leveled colored-noise sample.
    pub colored: f32,
    /// The sample chosen by [`Ps3300NoiseSettings::color`].
    pub selected: f32,
}

/// PS-3300 noise generator producing white and colored noise from a seeded RNG.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300Noise {
    settings: Ps3300NoiseSettings,
    rng: u32,
    colored: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300Noise {
    /// Builds a noise generator from sanitized settings, seeding the RNG.
    pub fn new(settings: Ps3300NoiseSettings) -> Self {
        let settings = sanitize(settings);
        Self {
            rng: nonzero_seed(settings.seed),
            settings,
            colored: 0.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the current (sanitized) settings.
    pub fn settings(&self) -> Ps3300NoiseSettings {
        self.settings
    }

    /// Advances the RNG one step and returns the white, colored, and selected samples.
    pub fn next_frame(&mut self) -> Ps3300NoiseFrame {
        let white = self.next_white();
        self.colored = one_pole(self.colored, white, self.settings.color_coefficient);
        let white = (white * self.settings.level).clamp(-1.0, 1.0);
        let colored = (self.colored * 2.6 * self.settings.level).clamp(-1.0, 1.0);
        let selected = match self.settings.color {
            Ps3300NoiseColor::White => white,
            Ps3300NoiseColor::Colored => colored,
        };
        let frame = Ps3300NoiseFrame {
            white,
            colored,
            selected,
        };
        self.last_trace = Some(self.trace_frame(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    /// Advances one step and returns only the selected-color sample.
    pub fn next_sample(&mut self) -> f32 {
        self.next_frame().selected
    }

    fn next_white(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let unit = self.rng as f32 / u32::MAX as f32;
        unit * 2.0 - 1.0
    }

    fn trace_frame(&self, frame: Ps3300NoiseFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_noise_component_id(),
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
            trace_key("colored"),
            ComponentTraceValue::Float(f64::from(frame.colored)),
        )
    }
}

impl Default for Ps3300Noise {
    fn default() -> Self {
        Self::new(Ps3300NoiseSettings::default())
    }
}

impl DiscreteComponent for Ps3300Noise {
    fn component_id(&self) -> Symbol {
        ps3_noise_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_noise_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_noise_params()
    }

    fn reset(&mut self) {
        self.rng = nonzero_seed(self.settings.seed);
        self.colored = 0.0;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let output = self.next_frame();
            write_output(block.out_audio, 0, frame, output.selected);
            write_output(block.out_audio, 1, frame, output.white);
            write_output(block.out_audio, 2, frame, output.colored);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_noise_component_id(),
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

/// Returns the registry component id for the PS-3300 noise module.
pub fn ps3_noise_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-noise")
}

/// Returns the noise module's output port descriptors (selected, white, colored, trace).
pub fn ps3_noise_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("white-out", ComponentPortMedia::AudioRate).optional(),
        output_port("colored-out", ComponentPortMedia::AudioRate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the noise module's parameter descriptors (color, level, coefficient, seed).
pub fn ps3_noise_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("color"), "Color", ComponentParamUnit::Unitless)
            .with_enum_values(
                vec![
                    Ps3300NoiseColor::White.symbol(),
                    Ps3300NoiseColor::Colored.symbol(),
                ],
                0,
            ),
        ComponentParamDescriptor::new(param_key("level"), "Level", ComponentParamUnit::Normalized)
            .with_range(ComponentParamRange::new(0.0, 1.0, 0.6)),
        ComponentParamDescriptor::new(
            param_key("color-coefficient"),
            "Color coefficient",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.01, 0.5, 0.08)),
        ComponentParamDescriptor::new(param_key("seed"), "Seed", ComponentParamUnit::RawInteger)
            .with_raw_default(i64::from(Ps3300NoiseSettings::default().seed)),
    ]
}

fn sanitize(settings: Ps3300NoiseSettings) -> Ps3300NoiseSettings {
    Ps3300NoiseSettings {
        color: settings.color,
        level: settings.level.clamp(0.0, 1.0),
        seed: nonzero_seed(settings.seed),
        color_coefficient: settings.color_coefficient.clamp(0.01, 0.5),
    }
}

fn one_pole(state: f32, input: f32, coefficient: f32) -> f32 {
    state + coefficient * (input - state)
}

fn nonzero_seed(seed: u32) -> u32 {
    if seed == 0 { 0x3300_0001 } else { seed }
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
    Symbol::qualified("audio-synth/ps3300-port", name)
}

fn param_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-param", name)
}

fn inspect_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-inspect", name)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300-trace", name)
}
