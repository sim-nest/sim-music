use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use super::common::{
    input, input_port, inspect_key, output_port, param_key, trace_key, write_outputs,
};
use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortMedia, ComponentPrepareConfig,
    ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Configuration for a [`System700Keyboard`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700KeyboardSettings {
    /// MIDI key number mapped to 0 V (the pitch reference).
    pub reference_key: u8,
    /// Pitch-bend range in octaves at full bend deflection.
    pub bend_depth_octaves: f32,
}

impl Default for System700KeyboardSettings {
    fn default() -> Self {
        Self {
            reference_key: 60,
            bend_depth_octaves: 1.0,
        }
    }
}

/// Outputs produced for one keyboard frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System700KeyboardFrame {
    /// Pitch control voltage in volts-per-octave for the active key plus bend.
    pub pitch_cv: f32,
    /// Whether a key is currently held (gate open).
    pub gate: bool,
    /// Whether this frame began a new note (gate onset or key change).
    pub trigger: bool,
}

/// Monophonic keyboard controller producing pitch CV, gate, and trigger.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Keyboard {
    settings: System700KeyboardSettings,
    last_gate: bool,
    last_key: Option<u8>,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Keyboard {
    /// Creates a keyboard from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700KeyboardSettings) -> Self {
        Self {
            settings: sanitize(settings),
            last_gate: false,
            last_key: None,
            clock: 0,
            last_trace: None,
        }
    }

    /// Maps a MIDI `key` and `bend` amount to a pitch control voltage.
    pub fn map_key(&self, key: u8, bend: f32) -> f32 {
        (key as f32 - self.settings.reference_key as f32) / 12.0
            + bend * self.settings.bend_depth_octaves
    }

    /// Produces the next keyboard frame, holding the last key when `key` is `None`.
    pub fn next_frame(&mut self, key: Option<u8>, gate: bool, bend: f32) -> System700KeyboardFrame {
        let active_key = key.or(self.last_key).unwrap_or(self.settings.reference_key);
        let trigger = gate && (!self.last_gate || Some(active_key) != self.last_key);
        let frame = System700KeyboardFrame {
            pitch_cv: self.map_key(active_key, bend).clamp(-10.0, 10.0),
            gate,
            trigger,
        };
        self.last_gate = gate;
        self.last_key = if gate { Some(active_key) } else { key };
        self.last_trace = Some(self.trace_frame(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: System700KeyboardFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_keyboard_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(trace_key("gate"), ComponentTraceValue::Bool(frame.gate))
        .with_state(
            trace_key("trigger"),
            ComponentTraceValue::Bool(frame.trigger),
        )
        .with_output(
            trace_key("pitch-cv"),
            ComponentTraceValue::Float(f64::from(frame.pitch_cv)),
        )
    }
}

impl Default for System700Keyboard {
    fn default() -> Self {
        Self::new(System700KeyboardSettings::default())
    }
}

impl DiscreteComponent for System700Keyboard {
    fn component_id(&self) -> Symbol {
        r700_keyboard_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_keyboard_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_keyboard_params()
    }

    fn reset(&mut self) {
        self.last_gate = false;
        self.last_key = None;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let key = input(block.in_audio, 0, frame).round().clamp(0.0, 127.0) as u8;
            let gate = input(block.in_audio, 1, frame) > 0.5;
            let mapped = self.next_frame(Some(key), gate, input(block.in_audio, 2, frame));
            write_outputs(
                block.out_audio,
                frame,
                &[
                    mapped.pitch_cv,
                    if mapped.gate { 1.0 } else { 0.0 },
                    if mapped.trigger { 1.0 } else { 0.0 },
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            r700_keyboard_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("reference-key"),
            self.settings.reference_key.to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 keyboard module.
pub fn r700_keyboard_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-keyboard")
}

/// Returns the port descriptors for the System 700 keyboard module.
pub fn r700_keyboard_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("key-in", ComponentPortMedia::Metadata),
        input_port("gate-in", ComponentPortMedia::Gate),
        input_port("bend-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("pitch-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 keyboard module.
pub fn r700_keyboard_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("reference-key"),
            "Reference key",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(i64::from(
            System700KeyboardSettings::default().reference_key,
        )),
        ComponentParamDescriptor::new(
            param_key("bend-depth-octaves"),
            "Bend depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 4.0, 1.0)),
    ]
}

fn sanitize(settings: System700KeyboardSettings) -> System700KeyboardSettings {
    System700KeyboardSettings {
        reference_key: settings.reference_key.min(127),
        bend_depth_octaves: settings.bend_depth_octaves.clamp(0.0, 4.0),
    }
}
