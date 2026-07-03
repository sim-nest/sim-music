//! PS-3300-style section generators and the three-section output summer.
//!
//! A PS-3300 has three identical sections, each pairing a tone source with a
//! polyphonic cell array. [`Ps3300SectionGenerator`] models one such section,
//! and [`Ps3300ThreeSectionSummer`] mixes all three sections plus the resonator
//! return into the final output. Both are [`DiscreteComponent`]s.

use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    ps3300::{
        Ps3300PolyArray, Ps3300PolyArraySettings, Ps3300Section, Ps3300ToneSource,
        Ps3300ToneSourceSettings, ps3300_keyboard_assignment,
    },
};

/// Fixture names for the section conformance scenarios (single-section chord
/// render, three-section summer stack).
pub const PS3300_SECTION_FIXTURE_NAMES: [&str; 2] = [
    "ps3300-ps3-section-chord-render",
    "ps3300-ps3-three-section-summer-stack",
];

/// Configuration for a [`Ps3300SectionGenerator`].
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300SectionGeneratorSettings {
    /// Which of the three PS-3300 sections this generator represents.
    pub section: Ps3300Section,
    /// Output level of the section, in 0.0..=1.0.
    pub level: f32,
    /// Settings of the section's tone source.
    pub tone: Ps3300ToneSourceSettings,
    /// Settings of the section's polyphonic cell array.
    pub poly: Ps3300PolyArraySettings,
}

impl Default for Ps3300SectionGeneratorSettings {
    fn default() -> Self {
        Self {
            section: Ps3300Section::A,
            level: 0.82,
            tone: Ps3300ToneSourceSettings::default(),
            poly: Ps3300PolyArraySettings::default(),
        }
    }
}

/// One rendered chord of a section generator.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300SectionFrame {
    /// Section that produced this frame.
    pub section: Ps3300Section,
    /// Number of cells active in the section's poly array.
    pub active_count: usize,
    /// Tone-source signal fed into the cell array.
    pub source: f32,
    /// Final section output after leveling, clamped to -1.0..=1.0.
    pub output: f32,
}

/// A PS-3300 section: a tone source feeding a polyphonic cell array.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300SectionGenerator {
    settings: Ps3300SectionGeneratorSettings,
    tone: Ps3300ToneSource,
    poly: Ps3300PolyArray,
    sample_rate_hz: f32,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300SectionGenerator {
    /// Builds a section generator from the given (sanitized) settings, wiring up
    /// its tone source and poly array at a default 48 kHz sample rate.
    pub fn new(settings: Ps3300SectionGeneratorSettings) -> Self {
        let settings = sanitize_section(settings);
        Self {
            tone: Ps3300ToneSource::new(settings.tone),
            poly: Ps3300PolyArray::new(settings.poly),
            settings,
            sample_rate_hz: 48_000.0,
            clock: 0,
            last_trace: None,
        }
    }

    /// Sets the working sample rate in Hz, propagating it to the tone source and
    /// poly array.
    pub fn set_sample_rate(&mut self, sample_rate_hz: f32) {
        self.sample_rate_hz = sample_rate_hz.max(1.0);
        self.tone.set_sample_rate(self.sample_rate_hz);
        self.poly.set_sample_rate(self.sample_rate_hz);
    }

    /// Renders one chord: drives the tone source from the held keys (plus pitch
    /// and modulation CV), passes the result through the poly array, and levels
    /// the section output.
    pub fn next_chord(
        &mut self,
        active_keys: &[u8],
        pitch_cv_v: f32,
        gate_high: bool,
        modulation_cv_v: f32,
    ) -> Ps3300SectionFrame {
        let keys = if gate_high { active_keys } else { &[] };
        let source = if keys.is_empty() {
            self.tone
                .next_sample(
                    ps3300_keyboard_assignment().first_midi_key,
                    pitch_cv_v + modulation_cv_v,
                    false,
                )
                .mixed
        } else {
            keys.iter()
                .map(|key| {
                    self.tone
                        .next_sample(*key, pitch_cv_v + modulation_cv_v, true)
                        .mixed
                })
                .sum::<f32>()
                / keys.len() as f32
        };
        let poly = self.poly.next_chord(source, keys);
        let frame = Ps3300SectionFrame {
            section: self.settings.section,
            active_count: poly.active_count,
            source,
            output: (poly.mixed * self.settings.level).clamp(-1.0, 1.0),
        };
        self.last_trace = Some(self.trace_frame(&frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: &Ps3300SectionFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_section_generator_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("section"),
            ComponentTraceValue::Text(frame.section.as_str().to_owned()),
        )
        .with_state(
            trace_key("active-count"),
            ComponentTraceValue::Float(frame.active_count as f64),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(frame.output)),
        )
    }
}

impl Default for Ps3300SectionGenerator {
    fn default() -> Self {
        Self::new(Ps3300SectionGeneratorSettings::default())
    }
}

impl DiscreteComponent for Ps3300SectionGenerator {
    fn component_id(&self) -> Symbol {
        ps3_section_generator_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_section_generator_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_section_generator_params()
    }

    fn reset(&mut self) {
        self.tone.reset();
        self.poly.reset();
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        self.set_sample_rate(config.sample_rate_hz as f32);
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let midi_key = input(block.in_audio, 0, frame).round().clamp(0.0, 127.0) as u8;
            let gate = input(block.in_audio, 1, frame) > 0.0;
            let output = self.next_chord(
                &[midi_key],
                input(block.in_audio, 2, frame),
                gate,
                input(block.in_audio, 3, frame),
            );
            write_outputs(block.out_audio, frame, &[output.output, output.source]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_section_generator_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("section"),
            self.settings.section.as_str().to_owned(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for a [`Ps3300ThreeSectionSummer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ThreeSectionSummerSettings {
    /// Per-section input gains, one per section (A, B, C).
    pub section_gains: [f32; 3],
    /// Gain applied to the resonator return before summing.
    pub resonator_gain: f32,
    /// Gain applied to the final mixed output.
    pub output_gain: f32,
}

impl Default for Ps3300ThreeSectionSummerSettings {
    fn default() -> Self {
        Self {
            section_gains: [0.8, 0.8, 0.8],
            resonator_gain: 1.0,
            output_gain: 0.8,
        }
    }
}

/// One rendered sample of the three-section summer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ThreeSectionSummerFrame {
    /// Gained sum of the three dry sections, before the resonator return.
    pub dry_sum: f32,
    /// Final mixed output after the resonator return and output gain.
    pub output: f32,
}

/// Output mixer that sums the three sections and the resonator return.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300ThreeSectionSummer {
    settings: Ps3300ThreeSectionSummerSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300ThreeSectionSummer {
    /// Builds a three-section summer from the given (sanitized) settings.
    pub fn new(settings: Ps3300ThreeSectionSummerSettings) -> Self {
        Self {
            settings: sanitize_summer(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Mixes one sample: applies per-section gains to the three section inputs,
    /// adds the gained resonator return, and applies the output gain.
    pub fn next_sample(
        &mut self,
        sections: [f32; 3],
        resonator: f32,
    ) -> Ps3300ThreeSectionSummerFrame {
        let dry_sum = sections
            .iter()
            .zip(self.settings.section_gains)
            .map(|(sample, gain)| sample * gain)
            .sum::<f32>()
            .clamp(-4.0, 4.0);
        let output = ((dry_sum + resonator * self.settings.resonator_gain)
            * self.settings.output_gain)
            .clamp(-1.0, 1.0);
        let frame = Ps3300ThreeSectionSummerFrame { dry_sum, output };
        self.last_trace = Some(self.summer_trace(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn summer_trace(&self, frame: Ps3300ThreeSectionSummerFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_output_mixer_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_output(
            trace_key("dry-sum"),
            ComponentTraceValue::Float(f64::from(frame.dry_sum)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(frame.output)),
        )
    }
}

impl Default for Ps3300ThreeSectionSummer {
    fn default() -> Self {
        Self::new(Ps3300ThreeSectionSummerSettings::default())
    }
}

impl DiscreteComponent for Ps3300ThreeSectionSummer {
    fn component_id(&self) -> Symbol {
        ps3_output_mixer_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_output_mixer_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_output_mixer_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_sample(
                [
                    input(block.in_audio, 0, frame),
                    input(block.in_audio, 1, frame),
                    input(block.in_audio, 2, frame),
                ],
                input(block.in_audio, 3, frame),
            );
            write_outputs(block.out_audio, frame, &[output.output, output.dry_sum]);
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_output_mixer_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("sections"), "3")
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component ids for the section family: section generator and
/// output mixer.
pub fn ps3300_section_module_ids() -> [Symbol; 2] {
    [
        ps3_section_generator_component_id(),
        ps3_output_mixer_component_id(),
    ]
}

/// Returns the fixture names for the section conformance scenarios.
pub fn ps3300_section_fixture_names() -> [&'static str; 2] {
    PS3300_SECTION_FIXTURE_NAMES
}

/// Returns the qualified component id for the section generator module.
pub fn ps3_section_generator_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-section-generator")
}

/// Returns the qualified component id for the output mixer module.
pub fn ps3_output_mixer_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-output-mixer")
}

/// Returns the section generator's ports: key, gate, pitch CV, and modulation
/// CV inputs plus audio and source outputs.
pub fn ps3_section_generator_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("key-in", ComponentPortMedia::Metadata),
        input_port("gate-in", ComponentPortMedia::Gate),
        input_port("pitch-cv-in", ComponentPortMedia::ControlVoltage),
        input_port("modulation-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("source-out", ComponentPortMedia::AudioRate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the section generator's parameters: section level and modulation
/// depth.
pub fn ps3_section_generator_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("section-level"),
            "Section level",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.82)),
        ComponentParamDescriptor::new(
            param_key("modulation-depth"),
            "Modulation depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-2.0, 2.0, 1.0)),
    ]
}

/// Returns the output mixer's ports: three section audio inputs and the
/// resonator return plus the mixed audio and dry-sum outputs.
pub fn ps3_output_mixer_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("section-a-audio", ComponentPortMedia::AudioRate),
        input_port("section-b-audio", ComponentPortMedia::AudioRate),
        input_port("section-c-audio", ComponentPortMedia::AudioRate),
        input_port("resonator-audio", ComponentPortMedia::AudioRate).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("dry-sum-out", ComponentPortMedia::AudioRate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the output mixer's parameters: the three section gains, resonator
/// gain, and output gain.
pub fn ps3_output_mixer_params() -> Vec<ComponentParamDescriptor> {
    vec![
        gain_param("section-a-gain", "Section A gain", 0.8),
        gain_param("section-b-gain", "Section B gain", 0.8),
        gain_param("section-c-gain", "Section C gain", 0.8),
        gain_param("resonator-gain", "Resonator gain", 1.0),
        gain_param("output-gain", "Output gain", 0.8),
    ]
}

fn gain_param(name: &'static str, label: &'static str, default: f64) -> ComponentParamDescriptor {
    ComponentParamDescriptor::new(param_key(name), label, ComponentParamUnit::Unitless)
        .with_range(ComponentParamRange::new(0.0, 2.0, default))
}

fn sanitize_section(settings: Ps3300SectionGeneratorSettings) -> Ps3300SectionGeneratorSettings {
    Ps3300SectionGeneratorSettings {
        section: settings.section,
        level: settings.level.clamp(0.0, 1.0),
        tone: settings.tone,
        poly: settings.poly,
    }
}

fn sanitize_summer(settings: Ps3300ThreeSectionSummerSettings) -> Ps3300ThreeSectionSummerSettings {
    Ps3300ThreeSectionSummerSettings {
        section_gains: settings.section_gains.map(|gain| gain.clamp(0.0, 2.0)),
        resonator_gain: settings.resonator_gain.clamp(0.0, 2.0),
        output_gain: settings.output_gain.clamp(0.0, 2.0),
    }
}

fn input(channels: &[&[f32]], channel: usize, frame: usize) -> f32 {
    channels
        .get(channel)
        .and_then(|samples| samples.get(frame))
        .copied()
        .unwrap_or(0.0)
}

fn write_outputs(outputs: &mut [&mut [f32]], frame: usize, samples: &[f32]) {
    for (channel, output) in outputs.iter_mut().enumerate() {
        output[frame] = samples
            .get(channel)
            .copied()
            .or_else(|| samples.last().copied())
            .unwrap_or(0.0);
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
