use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    ps3300::{Ps3300PinMatrixRoute, ps3300_default_pin_matrix_routes},
};

const PS3300_PIN_SOURCES: [&str; 10] = [
    "keyboard-pitch-cv",
    "keyboard-gate",
    "modulation-cv",
    "sample-hold-cv",
    "external-cv",
    "external-gate",
    "section-a-audio",
    "section-b-audio",
    "section-c-audio",
    "resonator-audio",
];

const PS3300_PIN_TARGETS: [&str; 11] = [
    "section-a-pitch-cv",
    "section-b-pitch-cv",
    "section-c-pitch-cv",
    "section-a-gate",
    "section-b-gate",
    "section-c-gate",
    "sample-hold-signal-in",
    "sample-hold-trigger-in",
    "resonator-audio-in",
    "resonator-formant-cv",
    "output-mixer-audio-in",
];

const PS3300_LEGAL_PIN_PAIRS: [(&str, &str); 23] = [
    ("keyboard-pitch-cv", "section-a-pitch-cv"),
    ("keyboard-pitch-cv", "section-b-pitch-cv"),
    ("keyboard-pitch-cv", "section-c-pitch-cv"),
    ("keyboard-gate", "section-a-gate"),
    ("keyboard-gate", "section-b-gate"),
    ("keyboard-gate", "section-c-gate"),
    ("keyboard-gate", "sample-hold-trigger-in"),
    ("modulation-cv", "section-a-pitch-cv"),
    ("modulation-cv", "section-b-pitch-cv"),
    ("modulation-cv", "section-c-pitch-cv"),
    ("modulation-cv", "sample-hold-signal-in"),
    ("modulation-cv", "resonator-formant-cv"),
    ("sample-hold-cv", "section-a-pitch-cv"),
    ("sample-hold-cv", "section-b-pitch-cv"),
    ("sample-hold-cv", "section-c-pitch-cv"),
    ("sample-hold-cv", "resonator-formant-cv"),
    ("external-cv", "sample-hold-signal-in"),
    ("external-gate", "sample-hold-trigger-in"),
    ("section-a-audio", "resonator-audio-in"),
    ("section-b-audio", "resonator-audio-in"),
    ("section-c-audio", "resonator-audio-in"),
    ("resonator-audio", "output-mixer-audio-in"),
    ("section-a-audio", "output-mixer-audio-in"),
];

/// Describes the legal shape of the PS-3300 pin matrix: its named sources,
/// targets, and the source-to-target pairs that may be patched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ps3300PinMatrixFormat {
    /// Names of every routable signal source on the matrix.
    pub sources: &'static [&'static str],
    /// Names of every routable signal target on the matrix.
    pub targets: &'static [&'static str],
    /// The `(source, target)` pairs that form legal routes.
    pub legal_pairs: &'static [(&'static str, &'static str)],
}

/// Per-frame source signals fed into the PS-3300 pin matrix.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ps3300PinMatrixInputs {
    /// Keyboard pitch control voltage, in volts.
    pub keyboard_pitch_cv: f32,
    /// Keyboard gate state (true while a key is held).
    pub keyboard_gate: bool,
    /// Modulation control voltage, in volts.
    pub modulation_cv: f32,
    /// Sample-and-hold control voltage, in volts.
    pub sample_hold_cv: f32,
    /// External control voltage, in volts.
    pub external_cv: f32,
    /// External gate state.
    pub external_gate: bool,
    /// Audio output of sections A, B, and C.
    pub section_audio: [f32; 3],
    /// Audio returned from the resonator bank.
    pub resonator_audio: f32,
}

/// Per-frame target signals produced by routing the matrix inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ps3300PinMatrixFrame {
    /// Pitch control voltage delivered to sections A, B, and C.
    pub section_pitch_cv: [f32; 3],
    /// Gate state delivered to sections A, B, and C.
    pub section_gate: [bool; 3],
    /// Signal routed into the sample-and-hold input.
    pub sample_hold_signal: f32,
    /// Trigger routed into the sample-and-hold input.
    pub sample_hold_trigger: bool,
    /// Audio routed into the resonator bank.
    pub resonator_audio: f32,
    /// Formant control voltage routed to the resonator bank.
    pub resonator_formant_cv: f32,
    /// Audio routed into the output mixer.
    pub output_audio: f32,
}

/// The PS-3300 modulation matrix: a set of validated routes that mixes source
/// signals into target buses each frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300PinMatrix {
    routes: Vec<Ps3300PinMatrixRoute>,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300PinMatrix {
    /// Builds a matrix from `routes`, returning an error if any route is not
    /// a legal, normalized source-to-target pair.
    pub fn new(routes: Vec<Ps3300PinMatrixRoute>) -> Result<Self> {
        validate_pin_matrix_routes(&routes)?;
        Ok(Self {
            routes,
            clock: 0,
            last_trace: None,
        })
    }

    /// Returns the configured routes.
    pub fn routes(&self) -> &[Ps3300PinMatrixRoute] {
        &self.routes
    }

    /// Routes one frame of `inputs` through every route, accumulating and
    /// clamping each target bus, and returns the resulting [`Ps3300PinMatrixFrame`].
    pub fn route(&mut self, inputs: Ps3300PinMatrixInputs) -> Ps3300PinMatrixFrame {
        let mut frame = Ps3300PinMatrixFrame::default();
        let mut gate_accum = [0.0; 3];
        let mut sample_trigger = 0.0;
        for route in &self.routes {
            let value = source_value(&inputs, route.source.as_str()) * route.amount;
            match route.target.as_str() {
                "section-a-pitch-cv" => frame.section_pitch_cv[0] += value,
                "section-b-pitch-cv" => frame.section_pitch_cv[1] += value,
                "section-c-pitch-cv" => frame.section_pitch_cv[2] += value,
                "section-a-gate" => gate_accum[0] += value,
                "section-b-gate" => gate_accum[1] += value,
                "section-c-gate" => gate_accum[2] += value,
                "sample-hold-signal-in" => frame.sample_hold_signal += value,
                "sample-hold-trigger-in" => sample_trigger += value,
                "resonator-audio-in" => frame.resonator_audio += value,
                "resonator-formant-cv" => frame.resonator_formant_cv += value,
                "output-mixer-audio-in" => frame.output_audio += value,
                _ => {}
            }
        }
        frame.section_gate = gate_accum.map(|value| value >= 0.5);
        frame.sample_hold_trigger = sample_trigger >= 0.5;
        frame.section_pitch_cv = frame.section_pitch_cv.map(|value| value.clamp(-10.0, 10.0));
        frame.sample_hold_signal = frame.sample_hold_signal.clamp(-10.0, 10.0);
        frame.resonator_formant_cv = frame.resonator_formant_cv.clamp(-10.0, 10.0);
        frame.resonator_audio = frame.resonator_audio.clamp(-4.0, 4.0);
        frame.output_audio = frame.output_audio.clamp(-4.0, 4.0);
        self.last_trace = Some(self.trace_frame(&frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: &Ps3300PinMatrixFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_pin_matrix_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("route-count"),
            ComponentTraceValue::Float(self.routes.len() as f64),
        )
        .with_output(
            trace_key("resonator-audio"),
            ComponentTraceValue::Float(f64::from(frame.resonator_audio)),
        )
        .with_output(
            trace_key("output-audio"),
            ComponentTraceValue::Float(f64::from(frame.output_audio)),
        )
    }
}

impl Default for Ps3300PinMatrix {
    fn default() -> Self {
        Self::new(ps3300_default_pin_matrix_routes()).expect("default PS-3300 matrix routes")
    }
}

impl DiscreteComponent for Ps3300PinMatrix {
    fn component_id(&self) -> Symbol {
        ps3_pin_matrix_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_pin_matrix_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_pin_matrix_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.route(Ps3300PinMatrixInputs {
                keyboard_pitch_cv: input(block.in_audio, 0, frame),
                keyboard_gate: input(block.in_audio, 1, frame) > 0.0,
                modulation_cv: input(block.in_audio, 2, frame),
                section_audio: [
                    input(block.in_audio, 3, frame),
                    input(block.in_audio, 4, frame),
                    input(block.in_audio, 5, frame),
                ],
                resonator_audio: input(block.in_audio, 6, frame),
                ..Ps3300PinMatrixInputs::default()
            });
            write_outputs(
                block.out_audio,
                frame,
                &[
                    output.section_pitch_cv[0],
                    if output.section_gate[0] { 1.0 } else { 0.0 },
                    output.resonator_audio,
                    output.output_audio,
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_pin_matrix_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(inspect_key("routes"), self.routes.len().to_string())
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the static [`Ps3300PinMatrixFormat`] describing the matrix's legal
/// sources, targets, and pairs.
pub fn ps3300_pin_matrix_format() -> Ps3300PinMatrixFormat {
    Ps3300PinMatrixFormat {
        sources: &PS3300_PIN_SOURCES,
        targets: &PS3300_PIN_TARGETS,
        legal_pairs: &PS3300_LEGAL_PIN_PAIRS,
    }
}

/// Returns the names of every routable pin matrix source.
pub fn ps3300_pin_matrix_source_names() -> &'static [&'static str] {
    &PS3300_PIN_SOURCES
}

/// Returns the names of every routable pin matrix target.
pub fn ps3300_pin_matrix_target_names() -> &'static [&'static str] {
    &PS3300_PIN_TARGETS
}

/// Returns true when `source` may legally be patched to `target`.
pub fn ps3300_pin_matrix_pair_is_legal(source: &str, target: &str) -> bool {
    PS3300_LEGAL_PIN_PAIRS
        .iter()
        .any(|(legal_source, legal_target)| *legal_source == source && *legal_target == target)
}

/// Validates that every route names a known source and target, forms a legal
/// pair, and carries a finite amount in `-1.0..=1.0`.
pub fn validate_pin_matrix_routes(routes: &[Ps3300PinMatrixRoute]) -> Result<()> {
    for route in routes {
        if !PS3300_PIN_SOURCES.contains(&route.source.as_str()) {
            return Err(route_error(format!(
                "unknown PS-3300 pin source {}",
                route.source
            )));
        }
        if !PS3300_PIN_TARGETS.contains(&route.target.as_str()) {
            return Err(route_error(format!(
                "unknown PS-3300 pin target {}",
                route.target
            )));
        }
        if !ps3300_pin_matrix_pair_is_legal(route.source.as_str(), route.target.as_str()) {
            return Err(route_error(format!(
                "illegal PS-3300 pin route {} -> {}",
                route.source, route.target
            )));
        }
        if !route.amount.is_finite() || !(-1.0..=1.0).contains(&route.amount) {
            return Err(route_error(format!(
                "PS-3300 pin route amount must be finite and normalized: {}",
                route.amount
            )));
        }
    }
    Ok(())
}

/// Returns the component id symbol for the PS-3300 pin matrix module.
pub fn ps3_pin_matrix_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-pin-matrix")
}

/// Returns the port descriptors for the PS-3300 pin matrix module.
pub fn ps3_pin_matrix_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("keyboard-pitch-cv", ComponentPortMedia::ControlVoltage),
        input_port("keyboard-gate", ComponentPortMedia::Gate),
        input_port("modulation-cv", ComponentPortMedia::ControlVoltage).optional(),
        input_port("section-a-audio", ComponentPortMedia::AudioRate).optional(),
        input_port("section-b-audio", ComponentPortMedia::AudioRate).optional(),
        input_port("section-c-audio", ComponentPortMedia::AudioRate).optional(),
        input_port("resonator-audio", ComponentPortMedia::AudioRate).optional(),
        output_port("section-a-pitch-cv", ComponentPortMedia::ControlVoltage),
        output_port("section-a-gate", ComponentPortMedia::Gate),
        output_port("resonator-audio-in", ComponentPortMedia::AudioRate),
        output_port("output-mixer-audio-in", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the PS-3300 pin matrix module.
pub fn ps3_pin_matrix_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("route-count"),
            "Route count",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(ps3300_default_pin_matrix_routes().len() as i64),
        ComponentParamDescriptor::new(
            param_key("amount"),
            "Route amount",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(-1.0, 1.0, 1.0)),
    ]
}

fn source_value(inputs: &Ps3300PinMatrixInputs, source: &str) -> f32 {
    match source {
        "keyboard-pitch-cv" => inputs.keyboard_pitch_cv,
        "keyboard-gate" => bool_value(inputs.keyboard_gate),
        "modulation-cv" => inputs.modulation_cv,
        "sample-hold-cv" => inputs.sample_hold_cv,
        "external-cv" => inputs.external_cv,
        "external-gate" => bool_value(inputs.external_gate),
        "section-a-audio" => inputs.section_audio[0],
        "section-b-audio" => inputs.section_audio[1],
        "section-c-audio" => inputs.section_audio[2],
        "resonator-audio" => inputs.resonator_audio,
        _ => 0.0,
    }
}

fn bool_value(value: bool) -> f32 {
    if value { 1.0 } else { 0.0 }
}

fn route_error(message: impl Into<String>) -> Error {
    Error::Eval(format!(
        "audio synth PS-3300 route error: {}",
        message.into()
    ))
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
