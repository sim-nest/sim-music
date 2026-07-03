use sim_kernel::{Result, Symbol};
use sim_lib_audio_graph_core::{
    BlockEvent, Graph as AudioGraph, PortDecl, PortDir, PortMedia, PrepareConfig, ProcessBlock,
    Processor,
};
use sim_lib_topology::Graph as TopologyGraph;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTick, ComponentTickResult, ComponentTraceFrame,
    ComponentTraceValue, DiscreteComponent, Dx7AlgorithmTopology, Dx7FmOperator,
    Dx7FmOperatorSettings, Dx7GraphInspection, Dx7ModeledOperator, Dx7OperatorInput, Dx7Patch,
    Dx7PatchOperator, QLevel, dx7_algorithm_topology_for_patch, dx7_component_id,
};

/// Live performance control state for a DX7 voice: the active note plus the
/// continuous controllers that shape it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dx7VoiceControl {
    /// MIDI channel (0..=15) the voice is responding to.
    pub channel: u8,
    /// Currently sounding MIDI key number.
    pub key: u8,
    /// Note velocity in 0.0..=1.0.
    pub velocity: f32,
    /// Whether the note gate is open (driving the operator envelopes).
    pub gate: bool,
    /// Whether the sustain pedal is held.
    pub sustain: bool,
    /// Pitch bend in semitones (clamped to -12.0..=12.0).
    pub pitch_bend_semitones: f32,
    /// Mod wheel position in 0.0..=1.0.
    pub mod_wheel: f32,
    /// Channel/key aftertouch pressure in 0.0..=1.0.
    pub aftertouch: f32,
}

impl Default for Dx7VoiceControl {
    /// Returns silent control state: middle C selected, gate closed, and all
    /// controllers centered or zeroed.
    fn default() -> Self {
        Self {
            channel: 0,
            key: 60,
            velocity: 0.0,
            gate: false,
            sustain: false,
            pitch_bend_semitones: 0.0,
            mod_wheel: 0.0,
            aftertouch: 0.0,
        }
    }
}

/// A complete monophonic DX7 voice: a patch realized as six FM operators
/// wired by an algorithm topology, driven by MIDI/control events and mixed to
/// audio.
#[derive(Clone, Debug)]
pub struct Dx7Voice {
    patch: Dx7Patch,
    backend: ComponentBackend,
    algorithm: &'static Dx7AlgorithmTopology,
    operators: Vec<Dx7VoiceOperator>,
    sample_rate_hz: u32,
    out_channels: u16,
    control: Dx7VoiceControl,
    held_key: Option<(u8, u8)>,
    sustained_key: Option<(u8, u8)>,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Dx7Voice {
    /// Builds a voice from `patch` on `backend`, instantiating one operator per
    /// node of the patch's algorithm and wiring feedback to the algorithm's
    /// feedback operator.
    pub fn new(patch: Dx7Patch, backend: ComponentBackend) -> Self {
        let algorithm = dx7_algorithm_topology_for_patch(patch.algorithm);
        let operators = algorithm
            .operator_order
            .iter()
            .map(|operator| {
                let patch_operator = patch
                    .operators
                    .get(usize::from(*operator - 1))
                    .cloned()
                    .unwrap_or_else(Dx7PatchOperator::default);
                let mut settings =
                    Dx7FmOperatorSettings::from_patch_operator(&patch_operator, &patch);
                settings.feedback = if algorithm
                    .feedback_edge
                    .is_some_and(|edge| edge.from_operator == *operator)
                {
                    patch.feedback.min(7)
                } else {
                    0
                };
                Dx7VoiceOperator::new(settings, backend)
            })
            .collect();

        Self {
            patch,
            backend,
            algorithm,
            operators,
            sample_rate_hz: 48_000,
            out_channels: 1,
            control: Dx7VoiceControl::default(),
            held_key: None,
            sustained_key: None,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the patch this voice was built from.
    pub fn patch(&self) -> &Dx7Patch {
        &self.patch
    }

    /// Returns the backend (algorithmic or modeled) the operators run on.
    pub fn backend(&self) -> ComponentBackend {
        self.backend
    }

    /// Returns the algorithm topology wiring the operators.
    pub fn algorithm(&self) -> &'static Dx7AlgorithmTopology {
        self.algorithm
    }

    /// Returns the current performance control state.
    pub fn control(&self) -> Dx7VoiceControl {
        self.control
    }

    /// Starts a note: latches the channel, key, and velocity, opens the gate
    /// when velocity is positive, and clears any sustained note.
    pub fn note_on(&mut self, channel: u8, key: u8, velocity: f32) {
        self.control.channel = channel & 0x0f;
        self.control.key = key;
        self.control.velocity = velocity.clamp(0.0, 1.0);
        self.control.gate = self.control.velocity > 0.0;
        self.held_key = Some((self.control.channel, key));
        self.sustained_key = None;
    }

    /// Releases the held note for `channel`/`key`: closes the gate, or defers
    /// the release to the sustained note when the sustain pedal is down.
    pub fn note_off(&mut self, channel: u8, key: u8) {
        let channel = channel & 0x0f;
        if self.held_key != Some((channel, key)) {
            return;
        }
        if self.control.sustain {
            self.sustained_key = self.held_key;
        } else {
            self.control.gate = false;
        }
        self.held_key = None;
    }

    /// Sets the sustain pedal state; releasing it closes the gate for any note
    /// that was held only by sustain.
    pub fn set_sustain(&mut self, sustain: bool) {
        self.control.sustain = sustain;
        if !sustain && self.sustained_key.take().is_some() && self.held_key.is_none() {
            self.control.gate = false;
        }
    }

    /// Sets the pitch bend in semitones, clamped to -12.0..=12.0.
    pub fn set_pitch_bend_semitones(&mut self, semitones: f32) {
        self.control.pitch_bend_semitones = semitones.clamp(-12.0, 12.0);
    }

    /// Sets the mod wheel position, clamped to 0.0..=1.0.
    pub fn set_mod_wheel(&mut self, value: f32) {
        self.control.mod_wheel = value.clamp(0.0, 1.0);
    }

    /// Sets the aftertouch pressure, clamped to 0.0..=1.0.
    pub fn set_aftertouch(&mut self, value: f32) {
        self.control.aftertouch = value.clamp(0.0, 1.0);
    }

    /// Returns a structural inspection of this voice's operator graph.
    pub fn graph_inspection(&self) -> Dx7GraphInspection {
        Dx7GraphInspection::new(self.algorithm, self.backend)
    }

    /// Returns the voice's algorithm as a generic topology graph.
    pub fn topology_graph(&self) -> TopologyGraph {
        self.algorithm.to_topology_graph(self.backend)
    }

    fn next_mono(&mut self) -> f32 {
        let mut outputs = [QLevel::ZERO; crate::DX7_OPERATOR_COUNT];
        for operator in self.algorithm.operator_order {
            let modulation = self.modulation_for_operator(operator, &outputs);
            let pitch_mod_semitones = self.pitch_mod_semitones();
            let output = self.operators[usize::from(operator - 1)].next_sample(Dx7OperatorInput {
                key: self.control.key,
                velocity: self.control.velocity,
                gate: self.control.gate,
                modulation,
                pitch_mod_semitones,
            });
            outputs[usize::from(operator - 1)] = output.sample;
        }

        let mut mix = 0.0;
        for carrier in self.algorithm.carrier_outputs() {
            mix += outputs[usize::from(carrier.operator - 1)].to_f32() * carrier.gain();
        }
        let sample = mix.clamp(-1.0, 1.0);
        self.last_trace = Some(self.trace_frame(sample));
        self.clock = self.clock.saturating_add(1);
        sample
    }

    fn modulation_for_operator(&self, operator: u8, outputs: &[QLevel; 6]) -> QLevel {
        let raw = self
            .algorithm
            .modulation_edges()
            .filter(|edge| edge.to_operator == operator)
            .map(|edge| {
                let source = outputs[usize::from(edge.from_operator - 1)].raw();
                let gain = self
                    .algorithm
                    .gain_for_operator(edge.gain_point)
                    .map(|gain| gain.gain_raw)
                    .unwrap_or(crate::DX7_GAIN_UNITY);
                i64::from(source) * i64::from(gain) / i64::from(crate::DX7_GAIN_UNITY)
            })
            .fold(0_i64, i64::saturating_add);
        QLevel::from_raw(clamp_i64_to_i32(raw))
    }

    fn pitch_mod_semitones(&self) -> f32 {
        self.control.pitch_bend_semitones
            + self.control.mod_wheel * 0.5
            + self.control.aftertouch * 0.25
    }

    fn handle_event(&mut self, event: BlockEvent<'_>) {
        match event {
            BlockEvent::NoteOn {
                channel,
                key,
                velocity,
                ..
            } if velocity > 0.0 => self.note_on(channel, key, velocity),
            BlockEvent::NoteOn { channel, key, .. } | BlockEvent::NoteOff { channel, key, .. } => {
                self.note_off(channel, key)
            }
            BlockEvent::Midi { bytes, len, .. } => self.handle_midi(&bytes[..usize::from(len)]),
            BlockEvent::MidiLong { bytes, .. } => self.handle_midi(bytes),
            BlockEvent::ParamSet { param, value, .. } => self.handle_param(param, value),
        }
    }

    fn handle_midi(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let status = bytes[0] & 0xf0;
        let channel = bytes[0] & 0x0f;
        match status {
            0x80 if bytes.len() >= 3 => self.note_off(channel, bytes[1]),
            0x90 if bytes.len() >= 3 && bytes[2] > 0 => {
                self.note_on(channel, bytes[1], f32::from(bytes[2]) / 127.0)
            }
            0x90 if bytes.len() >= 3 => self.note_off(channel, bytes[1]),
            0xa0 if bytes.len() >= 3 && self.held_key == Some((channel, bytes[1])) => {
                self.set_aftertouch(f32::from(bytes[2]) / 127.0);
            }
            0xb0 if bytes.len() >= 3 && bytes[1] == 1 => {
                self.set_mod_wheel(f32::from(bytes[2]) / 127.0);
            }
            0xb0 if bytes.len() >= 3 && bytes[1] == 64 => self.set_sustain(bytes[2] >= 64),
            0xd0 if bytes.len() >= 2 => self.set_aftertouch(f32::from(bytes[1]) / 127.0),
            0xe0 if bytes.len() >= 3 => {
                let raw = u16::from(bytes[1] & 0x7f) | (u16::from(bytes[2] & 0x7f) << 7);
                self.set_pitch_bend_semitones((f32::from(raw) - 8192.0) / 8192.0 * 2.0);
            }
            _ => {}
        }
    }

    fn handle_param(&mut self, param: u32, value: f64) {
        match param {
            1 => self.set_mod_wheel(value as f32),
            2 => self.set_pitch_bend_semitones((value as f32).clamp(-1.0, 1.0) * 2.0),
            64 => self.set_sustain(value >= 0.5),
            128 => self.set_aftertouch(value as f32),
            _ => {}
        }
    }

    fn trace_frame(&self, sample: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(dx7_component_id(), self.backend, self.clock)
            .with_integer(trace_key("algorithm"), i64::from(self.algorithm.id))
            .with_integer(trace_key("key"), i64::from(self.control.key))
            .with_input(
                trace_key("velocity"),
                ComponentTraceValue::Float(self.control.velocity as f64),
            )
            .with_state(
                trace_key("gate"),
                ComponentTraceValue::Bool(self.control.gate),
            )
            .with_state(
                trace_key("sustain"),
                ComponentTraceValue::Bool(self.control.sustain),
            )
            .with_output(
                trace_key("sample"),
                ComponentTraceValue::Float(sample as f64),
            )
    }
}

impl Default for Dx7Voice {
    /// Returns a voice built from the default patch on the algorithmic backend.
    fn default() -> Self {
        Self::new(Dx7Patch::default(), ComponentBackend::Algorithmic)
    }
}

impl Processor for Dx7Voice {
    fn prepare(&mut self, cfg: PrepareConfig) {
        self.sample_rate_hz = cfg.sample_rate_hz.max(1);
        self.out_channels = cfg.out_channels.max(1);
        for operator in &mut self.operators {
            operator.prepare(self.sample_rate_hz);
        }
    }

    fn reset(&mut self) {
        self.control = Dx7VoiceControl::default();
        self.held_key = None;
        self.sustained_key = None;
        self.clock = 0;
        self.last_trace = None;
        for operator in &mut self.operators {
            operator.reset();
        }
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            for event in block.in_events {
                if event_offset(*event) == frame as u32 {
                    self.handle_event(*event);
                }
            }
            let sample = self.next_mono();
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn ports(&self, _in_channels: u16, out_channels: u16) -> Vec<PortDecl> {
        vec![
            PortDecl::new("midi-in", PortMedia::Event, PortDir::In, 1),
            PortDecl::new(
                "audio-out",
                PortMedia::Audio,
                PortDir::Out,
                out_channels.max(1),
            ),
        ]
    }
}

impl DiscreteComponent for Dx7Voice {
    fn component_id(&self) -> Symbol {
        dx7_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        self.backend
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        dx7_voice_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        dx7_voice_params()
    }

    fn reset(&mut self) {
        Processor::reset(self);
    }

    fn prepare(&mut self, config: ComponentPrepareConfig) {
        Processor::prepare(self, config.into());
    }

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        Processor::process(self, block);
    }

    fn tick(&mut self, tick: ComponentTick) -> ComponentTickResult {
        self.control.gate = tick.gate;
        let output = self.next_mono();
        ComponentTickResult {
            output,
            trace: self.last_trace.clone(),
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(dx7_component_id(), self.backend, self.control.gate)
            .with_field(trace_key("algorithm"), self.algorithm.id.to_string())
            .with_field(trace_key("key"), self.control.key.to_string())
            .with_field(trace_key("velocity"), self.control.velocity.to_string())
            .with_field(
                trace_key("nodes"),
                self.graph_inspection().node_count.to_string(),
            )
            .with_field(
                trace_key("carriers"),
                self.algorithm.carrier_count().to_string(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

#[derive(Clone, Debug)]
enum Dx7VoiceOperator {
    Algorithmic(Dx7FmOperator),
    Modeled(Dx7ModeledOperator),
}

impl Dx7VoiceOperator {
    fn new(settings: Dx7FmOperatorSettings, backend: ComponentBackend) -> Self {
        match backend {
            ComponentBackend::Algorithmic => Self::Algorithmic(Dx7FmOperator::new(settings)),
            ComponentBackend::Modeled => Self::Modeled(Dx7ModeledOperator::new(settings)),
        }
    }

    fn prepare(&mut self, sample_rate_hz: u32) {
        match self {
            Self::Algorithmic(operator) => operator.set_sample_rate(sample_rate_hz as f32),
            Self::Modeled(operator) => operator.set_sample_rate(sample_rate_hz),
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Algorithmic(operator) => DiscreteComponent::reset(operator),
            Self::Modeled(operator) => DiscreteComponent::reset(operator),
        }
    }

    fn next_sample(&mut self, input: Dx7OperatorInput) -> Dx7OperatorOutputLike {
        match self {
            Self::Algorithmic(operator) => {
                let output = operator.next_sample(input);
                Dx7OperatorOutputLike {
                    sample: output.sample,
                }
            }
            Self::Modeled(operator) => {
                let output = operator.next_sample(input);
                Dx7OperatorOutputLike {
                    sample: output.sample,
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Dx7OperatorOutputLike {
    sample: QLevel,
}

/// Builds an audio graph containing a single DX7 voice node for `patch` on
/// `backend`, exposing one audio output.
pub fn dx7_voice_audio_graph(patch: Dx7Patch, backend: ComponentBackend) -> Result<AudioGraph> {
    let mut graph = AudioGraph::new();
    graph.add_node("dx7-voice", Box::new(Dx7Voice::new(patch, backend)), 0, 1)?;
    Ok(graph)
}

/// Returns the port descriptors for a DX7 voice: a MIDI input, an audio
/// output, and optional inspection and trace outputs.
pub fn dx7_voice_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "midi-in"),
            ComponentPortMedia::Event,
            ComponentPortDirection::Input,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "audio-out"),
            ComponentPortMedia::AudioRate,
            ComponentPortDirection::Output,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "inspection-out"),
            ComponentPortMedia::Metadata,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "trace-out"),
            ComponentPortMedia::Trace,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
    ]
}

/// Returns the parameter descriptors exposed by a DX7 voice: algorithm,
/// backend, pitch bend, mod wheel, and aftertouch.
pub fn dx7_voice_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "algorithm"),
            "Algorithm",
            ComponentParamUnit::RawInteger,
        )
        .with_range(ComponentParamRange::new(1.0, 32.0, 1.0))
        .with_raw_default(1),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "backend"),
            "Backend",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                ComponentBackend::Algorithmic.symbol(),
                ComponentBackend::Modeled.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "pitch-bend"),
            "Pitch bend",
            ComponentParamUnit::Semitones,
        )
        .with_range(ComponentParamRange::new(-12.0, 12.0, 0.0)),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "mod-wheel"),
            "Mod wheel",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.0)),
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "aftertouch"),
            "Aftertouch",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.0)),
    ]
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/dx7-voice-trace", name)
}

fn event_offset(event: BlockEvent<'_>) -> u32 {
    match event {
        BlockEvent::Midi { offset, .. }
        | BlockEvent::MidiLong { offset, .. }
        | BlockEvent::ParamSet { offset, .. }
        | BlockEvent::NoteOn { offset, .. }
        | BlockEvent::NoteOff { offset, .. } => offset,
    }
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
