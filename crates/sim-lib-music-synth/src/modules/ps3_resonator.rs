use std::f32::consts::PI;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Routing topology of the PS-3300 triple resonator bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300ResonatorMode {
    /// All three bandpass filters run from the same input and are summed.
    Parallel,
    /// The three bandpass filters are chained, each feeding the next.
    Series,
}

impl Ps3300ResonatorMode {
    /// Returns the stable string name of this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parallel => "parallel",
            Self::Series => "series",
        }
    }

    /// Returns the qualified [`Symbol`] naming this mode.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-resonator-mode", self.as_str())
    }
}

/// Tuning of a single resonator band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ResonatorBandSettings {
    /// Center frequency of the band, in hertz.
    pub center_hz: f32,
    /// Resonance (Q) of the band.
    pub q: f32,
    /// Output gain applied to the band.
    pub gain: f32,
}

impl Ps3300ResonatorBandSettings {
    /// Creates band settings from a center frequency, Q, and gain.
    pub const fn new(center_hz: f32, q: f32, gain: f32) -> Self {
        Self { center_hz, q, gain }
    }
}

/// Settings for the three-band PS-3300 resonator bank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300TripleResonatorSettings {
    /// Tuning of the low, mid, and high bands.
    pub bands: [Ps3300ResonatorBandSettings; 3],
    /// Whether the bands run in parallel or series.
    pub mode: Ps3300ResonatorMode,
    /// Octaves of band-center sweep per volt of formant CV.
    pub cv_depth_octaves: f32,
    /// Output level applied to the mixed signal.
    pub level: f32,
}

impl Default for Ps3300TripleResonatorSettings {
    fn default() -> Self {
        Self {
            bands: [
                Ps3300ResonatorBandSettings::new(720.0, 5.0, 0.9),
                Ps3300ResonatorBandSettings::new(1_440.0, 5.5, 0.85),
                Ps3300ResonatorBandSettings::new(2_880.0, 6.0, 0.8),
            ],
            mode: Ps3300ResonatorMode::Parallel,
            cv_depth_octaves: 1.0,
            level: 0.8,
        }
    }
}

/// One frame of resonator output: the per-band signals, the mixed output, and
/// the active band centers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ResonatorFrame {
    /// Output of the low band.
    pub low: f32,
    /// Output of the mid band.
    pub mid: f32,
    /// Output of the high band.
    pub high: f32,
    /// Mixed and leveled resonator output.
    pub output: f32,
    /// Active center frequencies of the three bands, in hertz.
    pub centers_hz: [f32; 3],
}

/// The PS-3300 triple resonator: three CV-swept bandpass filters mixed
/// according to the configured mode.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300TripleResonator {
    settings: Ps3300TripleResonatorSettings,
    sample_rate_hz: f32,
    filters: [BandpassState; 3],
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300TripleResonator {
    /// Creates a resonator with sanitized `settings` at the default sample rate.
    pub fn new(settings: Ps3300TripleResonatorSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            filters: [BandpassState::default(); 3],
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the current resonator settings.
    pub fn settings(&self) -> Ps3300TripleResonatorSettings {
        self.settings
    }

    /// Sets the working sample rate, clamped to at least 1 Hz.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the three band centers for a given formant CV, swept by
    /// `cv_depth_octaves` and clamped below Nyquist.
    pub fn band_centers_hz(&self, cv_v: f32) -> [f32; 3] {
        let ratio = 2.0_f32.powf(cv_v * self.settings.cv_depth_octaves);
        self.settings
            .bands
            .map(|band| (band.center_hz * ratio).clamp(20.0, self.sample_rate_hz * 0.45))
    }

    /// Processes one input sample with formant CV `cv_v` and returns the
    /// resulting [`Ps3300ResonatorFrame`].
    pub fn next_sample(&mut self, input: f32, cv_v: f32) -> Ps3300ResonatorFrame {
        let centers = self.band_centers_hz(cv_v);
        let mut band_outputs = [0.0; 3];
        match self.settings.mode {
            Ps3300ResonatorMode::Parallel => {
                for (index, filter) in self.filters.iter_mut().enumerate() {
                    band_outputs[index] = filter.next_bandpass(
                        input,
                        centers[index],
                        self.settings.bands[index].q,
                        self.sample_rate_hz,
                    ) * self.settings.bands[index].gain;
                }
            }
            Ps3300ResonatorMode::Series => {
                let mut signal = input;
                for (index, filter) in self.filters.iter_mut().enumerate() {
                    signal = filter.next_bandpass(
                        signal,
                        centers[index],
                        self.settings.bands[index].q,
                        self.sample_rate_hz,
                    ) * self.settings.bands[index].gain;
                    band_outputs[index] = signal;
                }
            }
        }
        let mixed = match self.settings.mode {
            Ps3300ResonatorMode::Parallel => {
                (band_outputs[0] + band_outputs[1] + band_outputs[2]) / 3.0
            }
            Ps3300ResonatorMode::Series => band_outputs[2],
        };
        let frame = Ps3300ResonatorFrame {
            low: band_outputs[0],
            mid: band_outputs[1],
            high: band_outputs[2],
            output: (mixed * self.settings.level).clamp(-1.0, 1.0),
            centers_hz: centers,
        };
        self.last_trace = Some(self.trace_frame(&frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: &Ps3300ResonatorFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_resonator_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("mode"),
            ComponentTraceValue::Text(self.settings.mode.as_str().to_owned()),
        )
        .with_state(
            trace_key("low-center-hz"),
            ComponentTraceValue::Float(f64::from(frame.centers_hz[0])),
        )
        .with_state(
            trace_key("mid-center-hz"),
            ComponentTraceValue::Float(f64::from(frame.centers_hz[1])),
        )
        .with_state(
            trace_key("high-center-hz"),
            ComponentTraceValue::Float(f64::from(frame.centers_hz[2])),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(frame.output)),
        )
    }
}

impl Default for Ps3300TripleResonator {
    fn default() -> Self {
        Self::new(Ps3300TripleResonatorSettings::default())
    }
}

impl DiscreteComponent for Ps3300TripleResonator {
    fn component_id(&self) -> Symbol {
        ps3_resonator_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_resonator_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_resonator_params()
    }

    fn reset(&mut self) {
        self.filters = [BandpassState::default(); 3];
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let output = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_output(block.out_audio, 0, frame, output.output);
            write_output(block.out_audio, 1, frame, output.low);
            write_output(block.out_audio, 2, frame, output.mid);
            write_output(block.out_audio, 3, frame, output.high);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_resonator_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("mode"), self.settings.mode.as_str().to_owned())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id symbol for the PS-3300 resonator bank module.
pub fn ps3_resonator_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-resonator-bank")
}

/// Returns the port descriptors for the PS-3300 resonator bank module.
pub fn ps3_resonator_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("formant-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("low-out", ComponentPortMedia::AudioRate).optional(),
        output_port("mid-out", ComponentPortMedia::AudioRate).optional(),
        output_port("high-out", ComponentPortMedia::AudioRate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the PS-3300 resonator bank module.
pub fn ps3_resonator_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("mode"), "Mode", ComponentParamUnit::Unitless)
            .with_enum_values(
                vec![
                    Ps3300ResonatorMode::Parallel.symbol(),
                    Ps3300ResonatorMode::Series.symbol(),
                ],
                0,
            ),
        ComponentParamDescriptor::new(
            param_key("low-center-hz"),
            "Low center",
            ComponentParamUnit::Hertz,
        )
        .with_range(ComponentParamRange::new(20.0, 8_000.0, 720.0)),
        ComponentParamDescriptor::new(
            param_key("mid-center-hz"),
            "Mid center",
            ComponentParamUnit::Hertz,
        )
        .with_range(ComponentParamRange::new(20.0, 12_000.0, 1_440.0)),
        ComponentParamDescriptor::new(
            param_key("high-center-hz"),
            "High center",
            ComponentParamUnit::Hertz,
        )
        .with_range(ComponentParamRange::new(20.0, 18_000.0, 2_880.0)),
        ComponentParamDescriptor::new(
            param_key("cv-depth-octaves"),
            "CV depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-2.0, 2.0, 1.0)),
        ComponentParamDescriptor::new(param_key("level"), "Level", ComponentParamUnit::Normalized)
            .with_range(ComponentParamRange::new(0.0, 1.0, 0.8)),
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BandpassState {
    low: f32,
    band: f32,
}

impl BandpassState {
    fn next_bandpass(&mut self, input: f32, center_hz: f32, q: f32, sample_rate_hz: f32) -> f32 {
        let f = (2.0 * (PI * center_hz / sample_rate_hz.max(1.0)).sin()).clamp(0.0, 1.0);
        let damping = (1.0 / q.max(0.1)).clamp(0.02, 2.0);
        self.low += f * self.band;
        let high = input - self.low - damping * self.band;
        self.band += f * high;
        self.band.clamp(-1.0, 1.0)
    }
}

fn sanitize(settings: Ps3300TripleResonatorSettings) -> Ps3300TripleResonatorSettings {
    Ps3300TripleResonatorSettings {
        bands: settings.bands.map(|band| Ps3300ResonatorBandSettings {
            center_hz: band.center_hz.clamp(20.0, 18_000.0),
            q: band.q.clamp(0.2, 40.0),
            gain: band.gain.clamp(0.0, 4.0),
        }),
        mode: settings.mode,
        cv_depth_octaves: settings.cv_depth_octaves.clamp(-2.0, 2.0),
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

fn write_output(channels: &mut [&mut [f32]], channel: usize, frame: usize, value: f32) {
    if let Some(samples) = channels.get_mut(channel)
        && let Some(sample) = samples.get_mut(frame)
    {
        *sample = value;
    }
}

fn input_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Input, 1)
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
