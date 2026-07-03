//! PS-3300-style top-octave-divider tone generator.
//!
//! Models the polyphonic tone source of the PS-3300: a bank of master
//! oscillators divided down per key, fanned out across the 16/8/4 footages,
//! detuned and band-limited by an aliasing policy, then mixed to a single
//! audio rail. The module exposes both the raw per-footage rails and the mix
//! through its [`DiscreteComponent`] ports.

use std::f32::consts::TAU;

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    ps3300::{PS3300_KEY_COUNT, ps3300_keyboard_assignment},
    voice::midi_key_to_hz,
};

/// Number of top-octave master oscillators feeding the divider chain, one per
/// semitone of the chromatic scale.
pub const PS3300_MASTER_OSCILLATOR_COUNT: usize = 12;
/// Fixture names for the four tone-generator conformance scenarios (pitch
/// coverage, footage transposition, divider determinism, aliasing policy).
pub const PS3300_TONE_SOURCE_FIXTURE_NAMES: [&str; 4] = [
    "ps3300-ps3-tonegen-pitch-coverage",
    "ps3300-ps3-tonegen-footage-transposition",
    "ps3300-ps3-tonegen-divider-determinism",
    "ps3300-ps3-tonegen-aliasing-policy",
];
const TOP_OCTAVE_FIRST_MIDI_KEY: u8 = 72;

/// Waveform shape produced by the tone generator oscillators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300ToneWaveform {
    /// Rising sawtooth ramp.
    Saw,
    /// Symmetric square wave (50% duty cycle).
    Square,
    /// Narrow pulse wave (32% duty cycle).
    Pulse,
    /// Pure sine wave.
    Sine,
}

impl Ps3300ToneWaveform {
    /// Returns the stable lowercase identifier for this waveform.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Saw => "saw",
            Self::Square => "square",
            Self::Pulse => "pulse",
            Self::Sine => "sine",
        }
    }

    /// Returns the qualified symbol naming this waveform as a parameter value.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-waveform", self.as_str())
    }
}

/// Organ-style footage (octave register) of a tone rail, named by pipe length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300Footage {
    /// 16' footage, sounding one octave below the played key.
    Sixteen,
    /// 8' footage, sounding at the played key.
    Eight,
    /// 4' footage, sounding one octave above the played key.
    Four,
}

impl Ps3300Footage {
    /// Returns the stable identifier for this footage ("16", "8", or "4").
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sixteen => "16",
            Self::Eight => "8",
            Self::Four => "4",
        }
    }

    /// Returns the qualified symbol naming this footage as a parameter value.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-footage", self.as_str())
    }

    /// Returns the octave shift this footage applies relative to 8' (0).
    pub fn octave_offset(self) -> i8 {
        match self {
            Self::Sixteen => -1,
            Self::Eight => 0,
            Self::Four => 1,
        }
    }

    /// Returns the frequency multiplier for this footage (a power of two).
    pub fn ratio(self) -> f32 {
        2.0_f32.powi(i32::from(self.octave_offset()))
    }
}

/// The three footages in fixed 16'/8'/4' order, used to index tone rails.
pub const PS3300_FOOTAGES: [Ps3300Footage; 3] = [
    Ps3300Footage::Sixteen,
    Ps3300Footage::Eight,
    Ps3300Footage::Four,
];

/// Strategy for handling oscillator frequencies that exceed Nyquist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300AliasingPolicy {
    /// Clamp the output frequency to the Nyquist limit.
    ClampToNyquist,
    /// Mirror (fold) over-Nyquist frequencies back below the limit.
    Foldback,
}

impl Ps3300AliasingPolicy {
    /// Returns the stable identifier for this aliasing policy.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClampToNyquist => "clamp-to-nyquist",
            Self::Foldback => "foldback",
        }
    }

    /// Returns the qualified symbol naming this policy as a parameter value.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-aliasing", self.as_str())
    }

    /// Applies the policy to `frequency_hz` for the given sample rate, returning
    /// the requested and band-limited frequencies plus an aliasing flag.
    pub fn apply(self, frequency_hz: f32, sample_rate_hz: f32) -> Ps3300AliasedFrequency {
        let requested_hz = frequency_hz.max(0.0);
        let nyquist_hz = (sample_rate_hz.max(1.0) * 0.5).max(1.0);
        let output_hz = match self {
            Self::ClampToNyquist => requested_hz.min(nyquist_hz),
            Self::Foldback => fold_frequency(requested_hz, nyquist_hz),
        };
        Ps3300AliasedFrequency {
            requested_hz,
            output_hz,
            aliased: requested_hz > nyquist_hz,
        }
    }
}

/// Result of applying an [`Ps3300AliasingPolicy`] to a requested frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300AliasedFrequency {
    /// Frequency requested before band-limiting, in Hz (clamped at 0).
    pub requested_hz: f32,
    /// Frequency actually used for oscillation after the policy, in Hz.
    pub output_hz: f32,
    /// True when the request exceeded Nyquist and was altered.
    pub aliased: bool,
}

/// Mix levels for the three footage rails of a tone source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300FootageLevels {
    /// Level of the 16' rail, in 0.0..=1.0.
    pub sixteen: f32,
    /// Level of the 8' rail, in 0.0..=1.0.
    pub eight: f32,
    /// Level of the 4' rail, in 0.0..=1.0.
    pub four: f32,
}

impl Default for Ps3300FootageLevels {
    fn default() -> Self {
        Self {
            sixteen: 0.65,
            eight: 0.9,
            four: 0.55,
        }
    }
}

impl Ps3300FootageLevels {
    /// Returns the mix level for the given footage rail.
    pub fn level_for(self, footage: Ps3300Footage) -> f32 {
        match footage {
            Ps3300Footage::Sixteen => self.sixteen,
            Ps3300Footage::Eight => self.eight,
            Ps3300Footage::Four => self.four,
        }
    }

    fn sanitized(self) -> Self {
        Self {
            sixteen: self.sixteen.clamp(0.0, 1.0),
            eight: self.eight.clamp(0.0, 1.0),
            four: self.four.clamp(0.0, 1.0),
        }
    }

    fn sum(self) -> f32 {
        self.sixteen + self.eight + self.four
    }
}

/// Configuration for a [`Ps3300ToneSource`] oscillator bank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ToneSourceSettings {
    /// Oscillator waveform shared by all footage rails.
    pub waveform: Ps3300ToneWaveform,
    /// Policy applied when an oscillator frequency exceeds Nyquist.
    pub aliasing_policy: Ps3300AliasingPolicy,
    /// Per-footage mix levels.
    pub footage_levels: Ps3300FootageLevels,
    /// Global detune applied to every rail, in cents (-100..=100).
    pub detune_cents: f32,
    /// Output level of the mixed signal, in 0.0..=1.0.
    pub level: f32,
}

impl Default for Ps3300ToneSourceSettings {
    fn default() -> Self {
        Self {
            waveform: Ps3300ToneWaveform::Saw,
            aliasing_policy: Ps3300AliasingPolicy::Foldback,
            footage_levels: Ps3300FootageLevels::default(),
            detune_cents: 0.0,
            level: 0.85,
        }
    }
}

/// Deterministic record of how a key is sourced from a master oscillator and
/// frequency divider, as in a top-octave-divider design.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300DividerPlan {
    /// MIDI key this plan resolves.
    pub midi_key: u8,
    /// Index of the master oscillator (0..12) that feeds the key.
    pub master_index: u8,
    /// Number of octave-halving divider stages applied to the master.
    pub divider_stage: u8,
    /// Frequency of the master oscillator before division, in Hz.
    pub master_frequency_hz: f32,
    /// Frequency at the key after division, in Hz.
    pub divided_frequency_hz: f32,
}

/// One rendered sample of a tone source: the three footage rails plus the mix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ToneFrame {
    /// 16' rail sample.
    pub sixteen: f32,
    /// 8' rail sample.
    pub eight: f32,
    /// 4' rail sample.
    pub four: f32,
    /// Mixed, leveled, and clamped output sample.
    pub mixed: f32,
}

/// Polyphonic top-octave-divider tone generator with three footage rails.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300ToneSource {
    settings: Ps3300ToneSourceSettings,
    sample_rate_hz: f32,
    phases: [f32; 3],
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300ToneSource {
    /// Builds a tone source from the given (sanitized) settings at a default
    /// 48 kHz sample rate.
    pub fn new(settings: Ps3300ToneSourceSettings) -> Self {
        Self {
            settings: sanitize(settings),
            sample_rate_hz: 48_000.0,
            phases: [0.0; 3],
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the current (sanitized) settings.
    pub fn settings(&self) -> Ps3300ToneSourceSettings {
        self.settings
    }

    /// Sets the working sample rate in Hz (floored at 1.0).
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
    }

    /// Returns the divider plan that sources the given MIDI key.
    pub fn divider_plan(&self, midi_key: u8) -> Ps3300DividerPlan {
        ps3300_tone_divider_plan(midi_key)
    }

    /// Computes the band-limited frequency for a key, footage, and pitch CV,
    /// folding in the configured detune.
    pub fn frequency_hz(
        &self,
        midi_key: u8,
        footage: Ps3300Footage,
        pitch_cv_v: f32,
    ) -> Ps3300AliasedFrequency {
        let detune_octaves = self.settings.detune_cents / 1_200.0;
        let requested =
            midi_key_to_hz(midi_key) * footage.ratio() * 2.0_f32.powf(pitch_cv_v + detune_octaves);
        self.settings
            .aliasing_policy
            .apply(requested, self.sample_rate_hz)
    }

    /// Advances the oscillators one sample, returning the footage rails and
    /// mix. When `gate_high` is false the source emits silence but still
    /// advances its clock and trace.
    pub fn next_sample(
        &mut self,
        midi_key: u8,
        pitch_cv_v: f32,
        gate_high: bool,
    ) -> Ps3300ToneFrame {
        if !gate_high {
            let frame = Ps3300ToneFrame {
                sixteen: 0.0,
                eight: 0.0,
                four: 0.0,
                mixed: 0.0,
            };
            self.last_trace = Some(self.trace_frame(midi_key, frame));
            self.clock = self.clock.saturating_add(1);
            return frame;
        }

        let mut outputs = [0.0; 3];
        for (index, footage) in PS3300_FOOTAGES.into_iter().enumerate() {
            let frequency = self.frequency_hz(midi_key, footage, pitch_cv_v);
            outputs[index] = wave_sample(self.settings.waveform, self.phases[index])
                * self.settings.footage_levels.level_for(footage);
            self.phases[index] =
                (self.phases[index] + frequency.output_hz / self.sample_rate_hz).fract();
        }
        let active_sum = self.settings.footage_levels.sum().max(1.0);
        let mixed = ((outputs[0] + outputs[1] + outputs[2]) / active_sum * self.settings.level)
            .clamp(-1.0, 1.0);
        let frame = Ps3300ToneFrame {
            sixteen: outputs[0],
            eight: outputs[1],
            four: outputs[2],
            mixed,
        };
        self.last_trace = Some(self.trace_frame(midi_key, frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, midi_key: u8, frame: Ps3300ToneFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_tonegen_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("midi-key"),
            ComponentTraceValue::Float(f64::from(midi_key)),
        )
        .with_state(
            trace_key("waveform"),
            ComponentTraceValue::Text(self.settings.waveform.as_str().to_owned()),
        )
        .with_state(
            trace_key("aliasing-policy"),
            ComponentTraceValue::Text(self.settings.aliasing_policy.as_str().to_owned()),
        )
        .with_output(
            trace_key("mixed"),
            ComponentTraceValue::Float(f64::from(frame.mixed)),
        )
    }
}

impl Default for Ps3300ToneSource {
    fn default() -> Self {
        Self::new(Ps3300ToneSourceSettings::default())
    }
}

impl DiscreteComponent for Ps3300ToneSource {
    fn component_id(&self) -> Symbol {
        ps3_tonegen_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_tonegen_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_tonegen_params()
    }

    fn reset(&mut self) {
        self.phases = [0.0; 3];
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let midi_key = midi_key_from_pitch_cv(input(block.in_audio, 0, frame));
            let pitch_cv = input(block.in_audio, 1, frame);
            let gate = input(block.in_audio, 2, frame) > 0.0;
            let output = self.next_sample(midi_key, pitch_cv, gate);
            write_output(block.out_audio, 0, frame, output.mixed);
            write_output(block.out_audio, 1, frame, output.sixteen);
            write_output(block.out_audio, 2, frame, output.eight);
            write_output(block.out_audio, 3, frame, output.four);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_tonegen_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("waveform"),
            self.settings.waveform.as_str().to_owned(),
        )
        .with_field(
            inspect_key("aliasing-policy"),
            self.settings.aliasing_policy.as_str().to_owned(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component ids registered by the tone-source family: the tone
/// generator and the companion noise source.
pub fn ps3300_tone_source_module_ids() -> [Symbol; 2] {
    [ps3_tonegen_component_id(), ps3_noise_component_id()]
}

/// Returns the fixture names for the tone source plus the noise source band
/// scenario.
pub fn ps3300_tone_source_fixture_names() -> [&'static str; 5] {
    [
        PS3300_TONE_SOURCE_FIXTURE_NAMES[0],
        PS3300_TONE_SOURCE_FIXTURE_NAMES[1],
        PS3300_TONE_SOURCE_FIXTURE_NAMES[2],
        PS3300_TONE_SOURCE_FIXTURE_NAMES[3],
        "ps3300-ps3-noise-white-colored-bands",
    ]
}

/// Returns the divider plan for every key of the PS-3300 keyboard, in pitch
/// order, exercising the full chromatic coverage of the divider chain.
pub fn ps3300_pitch_coverage() -> Vec<Ps3300DividerPlan> {
    let assignment = ps3300_keyboard_assignment();
    (0..PS3300_KEY_COUNT)
        .map(|offset| ps3300_tone_divider_plan(assignment.first_midi_key + offset as u8))
        .collect()
}

/// Resolves the deterministic master-oscillator and divider-stage plan for a
/// MIDI key under the top-octave-divider scheme.
pub fn ps3300_tone_divider_plan(midi_key: u8) -> Ps3300DividerPlan {
    let master_index = midi_key % PS3300_MASTER_OSCILLATOR_COUNT as u8;
    let master_midi_key = TOP_OCTAVE_FIRST_MIDI_KEY + master_index;
    let divider_stage = master_midi_key.saturating_sub(midi_key) / 12;
    let master_frequency_hz = midi_key_to_hz(master_midi_key);
    let divided_frequency_hz = master_frequency_hz / 2.0_f32.powi(i32::from(divider_stage));
    Ps3300DividerPlan {
        midi_key,
        master_index,
        divider_stage,
        master_frequency_hz,
        divided_frequency_hz,
    }
}

/// Returns the qualified component id for the tone generator module.
pub fn ps3_tonegen_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-tonegen")
}

/// Returns the port descriptors for the tone generator: pitch/gate inputs and
/// the mixed plus per-footage audio outputs.
pub fn ps3_tonegen_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("keyboard-pitch-cv-in", ComponentPortMedia::ControlVoltage),
        input_port("pitch-mod-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        input_port("gate-in", ComponentPortMedia::Gate).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("footage-16-out", ComponentPortMedia::AudioRate).optional(),
        output_port("footage-8-out", ComponentPortMedia::AudioRate).optional(),
        output_port("footage-4-out", ComponentPortMedia::AudioRate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the tone generator: waveform,
/// aliasing policy, level, detune, and the three footage levels.
pub fn ps3_tonegen_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("waveform"),
            "Waveform",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                Ps3300ToneWaveform::Saw.symbol(),
                Ps3300ToneWaveform::Square.symbol(),
                Ps3300ToneWaveform::Pulse.symbol(),
                Ps3300ToneWaveform::Sine.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(
            param_key("aliasing-policy"),
            "Aliasing policy",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                Ps3300AliasingPolicy::ClampToNyquist.symbol(),
                Ps3300AliasingPolicy::Foldback.symbol(),
            ],
            1,
        ),
        ComponentParamDescriptor::new(param_key("level"), "Level", ComponentParamUnit::Normalized)
            .with_range(ComponentParamRange::new(0.0, 1.0, 0.85)),
        ComponentParamDescriptor::new(
            param_key("detune-cents"),
            "Detune",
            ComponentParamUnit::Semitones,
        )
        .with_range(ComponentParamRange::new(-100.0, 100.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("footage-16-level"),
            "16 footage level",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.65)),
        ComponentParamDescriptor::new(
            param_key("footage-8-level"),
            "8 footage level",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.9)),
        ComponentParamDescriptor::new(
            param_key("footage-4-level"),
            "4 footage level",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.55)),
    ]
}

fn sanitize(settings: Ps3300ToneSourceSettings) -> Ps3300ToneSourceSettings {
    Ps3300ToneSourceSettings {
        waveform: settings.waveform,
        aliasing_policy: settings.aliasing_policy,
        footage_levels: settings.footage_levels.sanitized(),
        detune_cents: settings.detune_cents.clamp(-100.0, 100.0),
        level: settings.level.clamp(0.0, 1.0),
    }
}

fn wave_sample(waveform: Ps3300ToneWaveform, phase: f32) -> f32 {
    match waveform {
        Ps3300ToneWaveform::Saw => 2.0 * phase - 1.0,
        Ps3300ToneWaveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Ps3300ToneWaveform::Pulse => {
            if phase < 0.32 {
                1.0
            } else {
                -1.0
            }
        }
        Ps3300ToneWaveform::Sine => (TAU * phase).sin(),
    }
}

fn fold_frequency(frequency_hz: f32, nyquist_hz: f32) -> f32 {
    let period = nyquist_hz * 2.0;
    let folded = frequency_hz.rem_euclid(period);
    if folded > nyquist_hz {
        period - folded
    } else {
        folded
    }
}

fn midi_key_from_pitch_cv(pitch_cv_v: f32) -> u8 {
    let assignment = ps3300_keyboard_assignment();
    let offset = (pitch_cv_v * 12.0).round() as i16;
    let key = i16::from(assignment.first_midi_key) + offset;
    key.clamp(0, 127) as u8
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

fn ps3_noise_component_id() -> Symbol {
    crate::modules::ps3_noise::ps3_noise_component_id()
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
