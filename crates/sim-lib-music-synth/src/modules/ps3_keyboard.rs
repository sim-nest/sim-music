use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    ps3300::{PS3300_KEY_COUNT, ps3300_keyboard_assignment},
};

/// Describes how the keyboard maps MIDI keys onto its per-key gate bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ps3300KeyboardGateMapping {
    /// MIDI note number of the lowest mapped key.
    pub first_midi_key: u8,
    /// Number of contiguous keys mapped.
    pub key_count: usize,
    /// Width of the per-key gate bus.
    pub gate_bus_width: usize,
}

/// Configuration for the PS-3300 keyboard controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300KeyboardSettings {
    /// MIDI note number of the lowest playable key.
    pub first_midi_key: u8,
    /// Number of contiguous keys the controller spans.
    pub key_count: usize,
    /// Voltage emitted on the gate and trigger outputs when active.
    pub gate_voltage: f32,
}

impl Default for Ps3300KeyboardSettings {
    fn default() -> Self {
        let assignment = ps3300_keyboard_assignment();
        Self {
            first_midi_key: assignment.first_midi_key,
            key_count: assignment.key_count,
            gate_voltage: 1.0,
        }
    }
}

/// One rendered output frame from the keyboard controller.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300KeyboardFrame {
    /// Pitch control voltage (1V per octave) for the selected key.
    pub pitch_cv: f32,
    /// Whether any key is held (gate high).
    pub gate: bool,
    /// Whether the held-key set changed this frame (new trigger).
    pub trigger: bool,
    /// Normalized (sorted, deduplicated, in-range) active MIDI keys.
    pub active_keys: Vec<u8>,
    /// Per-key gate state across the controller's key span.
    pub per_key_gates: Vec<bool>,
}

/// PS-3300 keyboard controller: tracks held keys and emits pitch CV, gate, and triggers.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300KeyboardController {
    settings: Ps3300KeyboardSettings,
    per_key_gates: Vec<bool>,
    last_active_keys: Vec<u8>,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl Ps3300KeyboardController {
    /// Builds a controller from sanitized settings with an empty gate bus.
    pub fn new(settings: Ps3300KeyboardSettings) -> Self {
        let settings = sanitize(settings);
        Self {
            settings,
            per_key_gates: vec![false; settings.key_count],
            last_active_keys: Vec::new(),
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the controller's current key-to-gate-bus mapping.
    pub fn mapping(&self) -> Ps3300KeyboardGateMapping {
        Ps3300KeyboardGateMapping {
            first_midi_key: self.settings.first_midi_key,
            key_count: self.settings.key_count,
            gate_bus_width: self.per_key_gates.len(),
        }
    }

    /// Returns the current gate state for a given MIDI key, or `false` if out of range.
    pub fn gate_for(&self, midi_key: u8) -> bool {
        self.key_index(midi_key)
            .and_then(|index| self.per_key_gates.get(index))
            .copied()
            .unwrap_or(false)
    }

    /// Renders one frame for a single optional key, gated by `gate`.
    pub fn next_key(&mut self, key: Option<u8>, gate: bool) -> Ps3300KeyboardFrame {
        let active_keys = if gate {
            key.filter(|key| self.key_index(*key).is_some())
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        self.next_chord(&active_keys)
    }

    /// Renders one frame for a set of simultaneously held keys, updating the gate bus.
    pub fn next_chord(&mut self, active_keys: &[u8]) -> Ps3300KeyboardFrame {
        self.per_key_gates.fill(false);
        let mut normalized = active_keys
            .iter()
            .copied()
            .filter(|key| self.key_index(*key).is_some())
            .collect::<Vec<_>>();
        normalized.sort_unstable();
        normalized.dedup();
        for key in &normalized {
            if let Some(index) = self.key_index(*key) {
                self.per_key_gates[index] = true;
            }
        }
        let trigger = normalized != self.last_active_keys && !normalized.is_empty();
        let pitch_key = normalized
            .first()
            .copied()
            .unwrap_or(self.settings.first_midi_key);
        let frame = Ps3300KeyboardFrame {
            pitch_cv: self.pitch_cv(pitch_key),
            gate: !normalized.is_empty(),
            trigger,
            active_keys: normalized.clone(),
            per_key_gates: self.per_key_gates.clone(),
        };
        self.last_active_keys = normalized;
        self.last_trace = Some(self.trace_frame(&frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn pitch_cv(&self, midi_key: u8) -> f32 {
        (f32::from(midi_key) - f32::from(self.settings.first_midi_key)) / 12.0
    }

    fn key_index(&self, midi_key: u8) -> Option<usize> {
        if midi_key < self.settings.first_midi_key {
            return None;
        }
        let index = usize::from(midi_key - self.settings.first_midi_key);
        (index < self.settings.key_count).then_some(index)
    }

    fn trace_frame(&self, frame: &Ps3300KeyboardFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            ps3_keyboard_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(trace_key("gate"), ComponentTraceValue::Bool(frame.gate))
        .with_state(
            trace_key("trigger"),
            ComponentTraceValue::Bool(frame.trigger),
        )
        .with_state(
            trace_key("active-count"),
            ComponentTraceValue::Float(frame.active_keys.len() as f64),
        )
        .with_output(
            trace_key("pitch-cv"),
            ComponentTraceValue::Float(f64::from(frame.pitch_cv)),
        )
    }
}

impl Default for Ps3300KeyboardController {
    fn default() -> Self {
        Self::new(Ps3300KeyboardSettings::default())
    }
}

impl DiscreteComponent for Ps3300KeyboardController {
    fn component_id(&self) -> Symbol {
        ps3_keyboard_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        ps3_keyboard_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        ps3_keyboard_params()
    }

    fn reset(&mut self) {
        self.per_key_gates.fill(false);
        self.last_active_keys.clear();
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let key = input(block.in_audio, 0, frame).round().clamp(0.0, 127.0) as u8;
            let gate = input(block.in_audio, 1, frame) > 0.0;
            let output = self.next_key(Some(key), gate);
            write_outputs(
                block.out_audio,
                frame,
                &[
                    output.pitch_cv,
                    if output.gate {
                        self.settings.gate_voltage
                    } else {
                        0.0
                    },
                    if output.trigger {
                        self.settings.gate_voltage
                    } else {
                        0.0
                    },
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            ps3_keyboard_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("first-midi-key"),
            self.settings.first_midi_key.to_string(),
        )
        .with_field(
            inspect_key("key-count"),
            self.settings.key_count.to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the default keyboard controller's key-to-gate-bus mapping.
pub fn ps3300_keyboard_gate_mapping() -> Ps3300KeyboardGateMapping {
    Ps3300KeyboardController::default().mapping()
}

/// Returns the registry component id for the PS-3300 keyboard controller module.
pub fn ps3_keyboard_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-keyboard-controller")
}

/// Returns the keyboard module's port descriptors (key/gate in, pitch/gate/trigger out).
pub fn ps3_keyboard_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("key-in", ComponentPortMedia::Metadata),
        input_port("gate-in", ComponentPortMedia::Gate),
        output_port("pitch-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("trigger-out", ComponentPortMedia::Gate),
        output_port("per-key-gate-bus-out", ComponentPortMedia::Gate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the keyboard module's parameter descriptors (first key, key count, gate voltage).
pub fn ps3_keyboard_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("first-midi-key"),
            "First MIDI key",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(i64::from(Ps3300KeyboardSettings::default().first_midi_key)),
        ComponentParamDescriptor::new(
            param_key("key-count"),
            "Key count",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(PS3300_KEY_COUNT as i64),
        ComponentParamDescriptor::new(
            param_key("gate-voltage"),
            "Gate voltage",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 10.0, 1.0)),
    ]
}

fn sanitize(settings: Ps3300KeyboardSettings) -> Ps3300KeyboardSettings {
    let first_midi_key = settings.first_midi_key.min(127);
    let available = 128usize - usize::from(first_midi_key);
    Ps3300KeyboardSettings {
        first_midi_key,
        key_count: settings.key_count.clamp(1, available.min(PS3300_KEY_COUNT)),
        gate_voltage: settings.gate_voltage.clamp(0.0, 10.0),
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
