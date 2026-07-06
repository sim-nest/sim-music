use sim_kernel::{Error, Result, Symbol};
use sim_lib_music_core::{
    ControlEvent, FrozenPlayable, LaneDescriptor, LaneId, LaneKind, LaneTarget,
    MusicComponentRegistry, PlayContext, PlayEvent, PlayStream, Playable, PlayableDescriptor,
    PlayableShape, Tick,
};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, LatencyClass, RateContract, StreamDirection, StreamMedia,
    StreamMetadata, StreamValue,
};

use crate::{AdsrSettings, ComponentRegistry, LfoSettings, OscillatorKind};

mod generator;
use generator::*;

/// The rate at which a modulator emits samples, selecting the clock domain and
/// output spacing used when rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulationRate {
    /// Per audio sample (sample-exact rate).
    Audio,
    /// Control rate (a fraction of the tick clock).
    Control,
    /// Once per MIDI tick.
    Tick,
    /// Once per step (a coarser tick subdivision).
    Step,
    /// Once per upstream note event.
    PerNote,
}

impl ModulationRate {
    /// Returns the stable kebab-case identifier for this rate.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Control => "control",
            Self::Tick => "tick",
            Self::Step => "step",
            Self::PerNote => "per-note",
        }
    }

    /// Returns the stream clock domain this rate runs in.
    pub fn clock_domain(self) -> ClockDomain {
        match self {
            Self::Audio => ClockDomain::Sample,
            Self::Control => ClockDomain::Control,
            Self::Tick | Self::Step | Self::PerNote => ClockDomain::MidiTick,
        }
    }

    /// Returns the stream rate contract for this rate at the given audio sample
    /// rate (used only for the audio rate).
    pub fn rate_contract(self, sample_rate_hz: u32) -> RateContract {
        match self {
            Self::Audio => RateContract::sample_exact(Some(sample_rate_hz.max(1))),
            Self::Control => RateContract::control(),
            Self::Tick | Self::Step | Self::PerNote => RateContract::midi_tick(),
        }
    }

    /// Returns the latency class for this rate (evaluated at a nominal 48 kHz).
    pub fn latency_class(self) -> LatencyClass {
        self.rate_contract(48_000).latency_class()
    }
}

/// The coarse category of a [`ModulationTargetPath`], identifying which part of
/// the system a modulator drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModulationTargetScope {
    /// A player-level parameter.
    PlayerParameter,
    /// An instrument (synth) parameter.
    InstrumentParameter,
    /// A named control lane.
    Control,
    /// A note property (pitch or velocity).
    Note,
}

/// A fully resolved modulation destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModulationTargetPath {
    /// A player parameter named by symbol.
    PlayerParameter(Symbol),
    /// An instrument parameter named by symbol.
    InstrumentParameter(Symbol),
    /// A control lane named by symbol.
    Control(Symbol),
    /// The pitch of triggered notes.
    NotePitch,
    /// The velocity of triggered notes.
    NoteVelocity,
}

impl ModulationTargetPath {
    /// Builds a player-parameter target from a parameter name, qualifying it in
    /// the `music/player-param` namespace.
    pub fn player_parameter(name: impl Into<String>) -> Self {
        Self::PlayerParameter(Symbol::qualified("music/player-param", name.into()))
    }

    /// Builds an instrument-parameter target from a parameter name, qualifying
    /// it in the `audio-synth/param` namespace.
    pub fn instrument_parameter(name: impl Into<String>) -> Self {
        Self::InstrumentParameter(Symbol::qualified("audio-synth/param", name.into()))
    }

    /// Builds a control-lane target from a control name, qualifying it in the
    /// `music/control` namespace.
    pub fn control(name: impl Into<String>) -> Self {
        Self::Control(Symbol::qualified("music/control", name.into()))
    }

    /// Returns the scope category of this target.
    pub fn scope(&self) -> ModulationTargetScope {
        match self {
            Self::PlayerParameter(_) => ModulationTargetScope::PlayerParameter,
            Self::InstrumentParameter(_) => ModulationTargetScope::InstrumentParameter,
            Self::Control(_) => ModulationTargetScope::Control,
            Self::NotePitch | Self::NoteVelocity => ModulationTargetScope::Note,
        }
    }

    /// Returns the symbol used to address this target on a control lane.
    ///
    /// Parameter and control targets return their own symbol; note targets
    /// return a stable symbol in the `music/modulator-target` namespace.
    pub fn control_symbol(&self) -> Symbol {
        match self {
            Self::PlayerParameter(symbol)
            | Self::InstrumentParameter(symbol)
            | Self::Control(symbol) => symbol.clone(),
            Self::NotePitch => Symbol::qualified("music/modulator-target", "note-pitch"),
            Self::NoteVelocity => Symbol::qualified("music/modulator-target", "note-velocity"),
        }
    }

    /// Reports whether this target is a player parameter that some entry in
    /// `registry` actually declares.
    pub fn resolves_player_parameter(&self, registry: &MusicComponentRegistry) -> bool {
        let Self::PlayerParameter(target) = self else {
            return false;
        };
        registry.entries().any(|entry| {
            entry
                .descriptor()
                .params
                .iter()
                .any(|param| &param.id == target)
        })
    }

    /// Reports whether this target is an instrument parameter that some entry
    /// in `registry` actually declares.
    pub fn resolves_instrument_parameter(&self, registry: &ComponentRegistry) -> bool {
        let Self::InstrumentParameter(target) = self else {
            return false;
        };
        registry
            .entries()
            .any(|entry| entry.params().iter().any(|param| param.id() == target))
    }
}

/// A single breakpoint in an [`AutomationCurve`]: a value at a tick.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomationPoint {
    /// Tick position of the breakpoint.
    pub tick: i64,
    /// Value at the breakpoint.
    pub value: f32,
}

impl AutomationPoint {
    /// Builds a breakpoint at `tick` with `value`.
    pub fn new(tick: i64, value: f32) -> Self {
        Self { tick, value }
    }
}

/// A tick-keyed breakpoint curve that linearly interpolates between points,
/// holding flat before the first and after the last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AutomationCurve {
    points: Vec<AutomationPoint>,
}

impl AutomationCurve {
    /// Builds a curve from breakpoints, sorting them by tick.
    pub fn new(mut points: Vec<AutomationPoint>) -> Self {
        points.sort_by_key(|point| point.tick);
        Self { points }
    }

    /// Returns the breakpoints in ascending tick order.
    pub fn points(&self) -> &[AutomationPoint] {
        &self.points
    }

    /// Evaluates the curve at `tick`, interpolating linearly between adjacent
    /// breakpoints and holding the endpoints outside the range. An empty curve
    /// reads as `0.0`.
    pub fn value_at(&self, tick: i64) -> f32 {
        let Some(first) = self.points.first() else {
            return 0.0;
        };
        if tick <= first.tick {
            return first.value;
        }
        for window in self.points.windows(2) {
            let left = &window[0];
            let right = &window[1];
            if tick <= right.tick {
                let width = (right.tick - left.tick).max(1) as f32;
                let frac = (tick - left.tick) as f32 / width;
                return left.value * (1.0 - frac) + right.value * frac;
            }
        }
        self.points.last().map(|point| point.value).unwrap_or(0.0)
    }
}

/// Settings for a bounded random-walk modulator source.
#[derive(Clone, Debug, PartialEq)]
pub struct RandomWalkSettings {
    /// Starting value, clamped into `[min, max]`.
    pub start: f32,
    /// Magnitude of each step (its absolute value is used).
    pub step: f32,
    /// Lower bound of the walk.
    pub min: f32,
    /// Upper bound of the walk.
    pub max: f32,
}

impl Default for RandomWalkSettings {
    fn default() -> Self {
        Self {
            start: 0.0,
            step: 0.1,
            min: -1.0,
            max: 1.0,
        }
    }
}

/// The signal generator that drives a modulator.
#[derive(Clone, Debug, PartialEq)]
pub enum ModulatorSource {
    /// A low-frequency oscillator with the given settings.
    Lfo(LfoSettings),
    /// An ADSR envelope with the given settings.
    Envelope(AdsrSettings),
    /// A phase oscillator running at a fixed frequency and amplitude.
    Oscillator {
        /// Oscillator waveform kind.
        kind: OscillatorKind,
        /// Oscillator frequency, in hertz.
        frequency_hz: f32,
        /// Output amplitude scaling.
        amplitude: f32,
    },
    /// A bounded random walk with the given settings.
    RandomWalk(RandomWalkSettings),
    /// A precomputed automation breakpoint curve.
    AutomationCurve(AutomationCurve),
}

/// The configuration of one modulator: its identity, source, target, rate, and
/// random seed.
#[derive(Clone, Debug, PartialEq)]
pub struct ModulatorConfig {
    /// Identifying symbol for the modulator.
    pub id: Symbol,
    /// The generator producing the modulator's signal.
    pub source: ModulatorSource,
    /// The destination the modulator drives.
    pub target: ModulationTargetPath,
    /// The emission rate of the modulator.
    pub rate: ModulationRate,
    /// Seed for stochastic sources (mixed with the context seed).
    pub seed: u64,
}

impl ModulatorConfig {
    /// Builds a config with the given id, source, target, and rate and a zero
    /// seed.
    pub fn new(
        id: Symbol,
        source: ModulatorSource,
        target: ModulationTargetPath,
        rate: ModulationRate,
    ) -> Self {
        Self {
            id,
            source,
            target,
            rate,
            seed: 0,
        }
    }

    /// Sets the random seed and returns the config (builder style).
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

/// One rendered modulator output: a value for a target at a tick.
#[derive(Clone, Debug, PartialEq)]
pub struct ModulatorSample {
    /// Tick the sample applies at.
    pub tick: Tick,
    /// Destination the sample drives.
    pub target: ModulationTargetPath,
    /// Sampled modulator value.
    pub value: f32,
}

/// A post-processing operator applied in sequence over a modulator's samples.
#[derive(Clone, Debug, PartialEq)]
pub enum ModulationOperator {
    /// Adds a constant offset to each sample.
    Sum(f32),
    /// Multiplies each sample by a factor.
    Multiply(f32),
    /// Holds each sampled value for a run of `samples` outputs.
    SampleHold {
        /// Number of outputs each held value spans (floored to one).
        samples: usize,
    },
    /// Quantizes each value to the nearest multiple of `step`.
    Quantize {
        /// Quantization grid size (its absolute value is used; zero disables).
        step: f32,
    },
    /// One-pole smoothing toward each new value.
    Smooth {
        /// Smoothing coefficient in `0.0..=1.0` (1.0 passes through).
        amount: f32,
    },
    /// Clips each value into `[min, max]`.
    Clip {
        /// Lower clip bound.
        min: f32,
        /// Upper clip bound.
        max: f32,
    },
    /// Slew limiting: bounds the per-sample change to `step`.
    Lag {
        /// Maximum change per sample (its absolute value is used).
        step: f32,
    },
}

/// A modulator paired with an ordered list of operators applied to its output.
#[derive(Clone, Debug, PartialEq)]
pub struct ModulationChain {
    /// The modulator producing the base samples.
    pub source: ModulatorConfig,
    /// Operators applied in order to the rendered samples.
    pub operators: Vec<ModulationOperator>,
}

impl ModulationChain {
    /// Builds a chain from a source config and its operators.
    pub fn new(source: ModulatorConfig, operators: Vec<ModulationOperator>) -> Self {
        Self { source, operators }
    }

    /// Renders the source's samples and applies each operator in order.
    pub fn render_samples(&self, cx: &PlayContext) -> Vec<ModulatorSample> {
        let mut samples = ModulatorPlayable::new(self.source.clone()).render_samples(cx);
        for operator in &self.operators {
            apply_operator(&mut samples, operator);
        }
        samples
    }
}

/// A [`Playable`] wrapper around a [`ModulatorConfig`] that renders the
/// modulator as a control-lane stream of events.
#[derive(Clone, Debug, PartialEq)]
pub struct ModulatorPlayable {
    /// The modulator configuration this playable renders.
    pub config: ModulatorConfig,
}

impl ModulatorPlayable {
    /// Wraps `config` in a playable.
    pub fn new(config: ModulatorConfig) -> Self {
        Self { config }
    }

    /// Renders the modulator's samples over the context's range, one per output
    /// tick determined by the configured rate.
    pub fn render_samples(&self, cx: &PlayContext) -> Vec<ModulatorSample> {
        let ticks = output_ticks(&self.config, cx);
        let mut generator = ModulatorGenerator::new(&self.config, cx);
        ticks
            .into_iter()
            .map(|tick| ModulatorSample {
                value: generator.next(tick.ticks),
                tick,
                target: self.config.target.clone(),
            })
            .collect()
    }

    fn render_events(&self, cx: &PlayContext) -> Vec<PlayEvent> {
        self.render_samples(cx)
            .into_iter()
            .map(|sample| {
                PlayEvent::Control(ControlEvent {
                    lane_id: LaneId::new("modulator"),
                    time: sample.tick,
                    control: sample.target.control_symbol(),
                    value: scaled_control(sample.value),
                })
            })
            .collect()
    }
}

impl Playable for ModulatorPlayable {
    fn describe(&self) -> Result<PlayableDescriptor> {
        Ok(PlayableDescriptor {
            id: self.config.id.clone(),
            lanes: vec![
                LaneDescriptor::new(
                    LaneId::new("modulator"),
                    LaneKind::Control,
                    LaneTarget::Control(self.config.target.control_symbol()),
                    0,
                )
                .map_err(|err| Error::Eval(err.to_string()))?,
            ],
            clock_domain: self.config.rate.clock_domain(),
            latency_class: self.config.rate.latency_class(),
            shape: PlayableShape::music_object(),
        })
    }

    fn render_range(&self, cx: &PlayContext) -> Result<PlayStream> {
        let mut events = self.render_events(cx);
        sim_lib_music_core::stable_event_order(&mut events);
        let clock = self.config.rate.clock_domain().symbol();
        let items = events
            .iter()
            .map(|event| event.to_stream_item(clock.clone()))
            .collect::<Result<Vec<_>>>()?;
        let metadata = StreamMetadata::new(
            self.config.id.clone(),
            StreamMedia::Data,
            StreamDirection::Source,
            clock,
            BufferPolicy::bounded(items.len().max(1))?,
        );
        Ok(StreamValue::pull(metadata, items))
    }

    fn freeze(&self, cx: &PlayContext) -> Result<FrozenPlayable> {
        let mut events = self.render_events(cx);
        sim_lib_music_core::stable_event_order(&mut events);
        Ok(FrozenPlayable {
            descriptor: self.describe()?,
            content_hash: modulator_hash(&self.config, &events, cx.seed),
            events,
        })
    }
}

/// Builds a [`ModulatorPlayable`] driven by an LFO with the given settings,
/// target, and rate.
pub fn lfo_modulator(
    id: Symbol,
    settings: LfoSettings,
    target: ModulationTargetPath,
    rate: ModulationRate,
) -> ModulatorPlayable {
    ModulatorPlayable::new(ModulatorConfig::new(
        id,
        ModulatorSource::Lfo(settings),
        target,
        rate,
    ))
}
