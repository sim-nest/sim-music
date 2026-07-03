use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
    GateConvention, GateConverter,
};

/// Configuration for the System 55 ribbon controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55RibbonSettings {
    /// Total pitch span in octaves swept across the ribbon's length.
    pub range_octaves: f32,
    /// Control voltage emitted at the center of the ribbon.
    pub center_cv_v: f32,
}

impl Default for System55RibbonSettings {
    fn default() -> Self {
        Self {
            range_octaves: 4.0,
            center_cv_v: 0.0,
        }
    }
}

/// Per-sample output of the System 55 ribbon controller.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55RibbonFrame {
    /// Pitch control voltage derived from the touch position.
    pub pitch_cv: f32,
    /// Pressure control voltage derived from the touch pressure.
    pub pressure_cv: f32,
    /// Whether the ribbon is currently being pressed.
    pub gate: bool,
    /// Whether the press just began this sample.
    pub trigger: bool,
}

/// System 55 ribbon controller that maps touch position and pressure to
/// pitch and pressure control voltages plus a gate.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Ribbon {
    settings: System55RibbonSettings,
    last_gate: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Ribbon {
    /// Creates a ribbon controller from sanitized settings.
    pub fn new(settings: System55RibbonSettings) -> Self {
        Self {
            settings: sanitize_ribbon(settings),
            last_gate: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Advances by one sample, mapping `position` and `pressure` to control
    /// voltages and a gate.
    pub fn next_frame(&mut self, position: f32, pressure: f32) -> System55RibbonFrame {
        let gate = pressure > 0.01;
        let frame = System55RibbonFrame {
            pitch_cv: (self.settings.center_cv_v
                + (position.clamp(0.0, 1.0) - 0.5) * self.settings.range_octaves)
                .clamp(-10.0, 10.0),
            pressure_cv: pressure.clamp(0.0, 1.0) * 5.0,
            gate,
            trigger: gate && !self.last_gate,
        };
        self.last_gate = gate;
        self.last_trace = Some(trace_output(
            m55_ribbon_component_id(),
            self.clock,
            frame.pitch_cv,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }
}

impl Default for System55Ribbon {
    fn default() -> Self {
        Self::new(System55RibbonSettings::default())
    }
}

impl DiscreteComponent for System55Ribbon {
    fn component_id(&self) -> Symbol {
        m55_ribbon_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_ribbon_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_ribbon_params()
    }

    fn reset(&mut self) {
        self.last_gate = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let output = self.next_frame(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(
                block.out_audio,
                frame,
                &[
                    output.pitch_cv,
                    output.pressure_cv,
                    GateConvention::s_trigger().native_voltage(output.gate),
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_ribbon_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("range-octaves"),
            self.settings.range_octaves.to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Configuration for the System 55 keyboard controller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55KeyboardSettings {
    /// MIDI key number mapped to 0 V pitch.
    pub reference_key: u8,
    /// Pitch-bend depth in octaves at full bend deflection.
    pub bend_depth_octaves: f32,
}

impl Default for System55KeyboardSettings {
    fn default() -> Self {
        Self {
            reference_key: 60,
            bend_depth_octaves: 1.0,
        }
    }
}

/// Per-sample output of the System 55 keyboard controller.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System55KeyboardFrame {
    /// Pitch control voltage for the active key plus bend.
    pub pitch_cv: f32,
    /// S-trigger output voltage reflecting the gate state.
    pub s_trigger_v: f32,
    /// Whether a new key strike occurred this sample.
    pub trigger: bool,
}

/// System 55 keyboard controller that converts key, gate, and bend inputs
/// into a one-volt-per-octave pitch and triggers.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Keyboard {
    settings: System55KeyboardSettings,
    last_gate: bool,
    last_key: Option<u8>,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Keyboard {
    /// Creates a keyboard controller from sanitized settings.
    pub fn new(settings: System55KeyboardSettings) -> Self {
        Self {
            settings: sanitize_keyboard(settings),
            last_gate: false,
            last_key: None,
            clock: 0,
            last_trace: None,
        }
    }

    /// Maps a MIDI key and bend amount to a clamped pitch control voltage.
    pub fn map_key(&self, key: u8, bend: f32) -> f32 {
        ((key as f32 - self.settings.reference_key as f32) / 12.0
            + bend * self.settings.bend_depth_octaves)
            .clamp(-10.0, 10.0)
    }

    /// Advances by one sample, tracking the held key and emitting pitch, gate,
    /// and trigger outputs.
    pub fn next_frame(&mut self, key: Option<u8>, gate: bool, bend: f32) -> System55KeyboardFrame {
        let active_key = key.or(self.last_key).unwrap_or(self.settings.reference_key);
        let trigger = gate && (!self.last_gate || Some(active_key) != self.last_key);
        let frame = System55KeyboardFrame {
            pitch_cv: self.map_key(active_key, bend),
            s_trigger_v: GateConvention::s_trigger().native_voltage(gate),
            trigger,
        };
        self.last_gate = gate;
        self.last_key = if gate { Some(active_key) } else { key };
        self.last_trace = Some(trace_output(
            m55_keyboard_component_id(),
            self.clock,
            frame.pitch_cv,
        ));
        self.clock = self.clock.saturating_add(1);
        frame
    }
}

impl Default for System55Keyboard {
    fn default() -> Self {
        Self::new(System55KeyboardSettings::default())
    }
}

impl DiscreteComponent for System55Keyboard {
    fn component_id(&self) -> Symbol {
        m55_keyboard_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_keyboard_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_keyboard_params()
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
            let output = self.next_frame(Some(key), gate, input(block.in_audio, 2, frame));
            write_outputs(
                block.out_audio,
                frame,
                &[
                    output.pitch_cv,
                    output.s_trigger_v,
                    if output.trigger { 0.0 } else { 5.0 },
                ],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_keyboard_component_id(),
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

/// System 55 interface that translates between S-trigger and voltage-gate
/// conventions in both directions.
#[derive(Clone, Debug, PartialEq)]
pub struct System55Interface {
    s_trigger: GateConverter,
    voltage_gate: GateConverter,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55Interface {
    /// Creates an interface with fresh S-trigger and voltage-gate converters.
    pub fn new() -> Self {
        Self {
            s_trigger: GateConverter::new(GateConvention::s_trigger()),
            voltage_gate: GateConverter::new(GateConvention::voltage_gate()),
            clock: 0,
            last_trace: None,
        }
    }

    /// Advances by one sample, returning the voltage-gate output, S-trigger
    /// output, and a combined triggered flag.
    pub fn next_frame(&mut self, s_trigger_v: f32, voltage_gate_v: f32) -> (f32, f32, bool) {
        let s_frame = self.s_trigger.convert(s_trigger_v);
        let v_frame = self.voltage_gate.convert(voltage_gate_v);
        let voltage_gate_out = s_frame.voltage_gate_volts;
        let s_trigger_out = GateConvention::s_trigger().native_voltage(v_frame.active);
        let triggered = s_frame.triggered || v_frame.triggered;
        self.last_trace = Some(trace_bool_output(
            m55_interface_component_id(),
            self.clock,
            triggered,
        ));
        self.clock = self.clock.saturating_add(1);
        (voltage_gate_out, s_trigger_out, triggered)
    }
}

impl Default for System55Interface {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscreteComponent for System55Interface {
    fn component_id(&self) -> Symbol {
        m55_interface_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_interface_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_interface_params()
    }

    fn reset(&mut self) {
        self.s_trigger.reset();
        self.voltage_gate.reset();
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            let (voltage_gate, s_trigger, triggered) = self.next_frame(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            write_outputs(
                block.out_audio,
                frame,
                &[voltage_gate, s_trigger, if triggered { 1.0 } else { 0.0 }],
            );
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_interface_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the stable component id for the ribbon controller module.
pub fn m55_ribbon_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-956-ribbon-controller")
}

/// Returns the stable component id for the keyboard controller module.
pub fn m55_keyboard_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-951-keyboard-controller")
}

/// Returns the stable component id for the interface module.
pub fn m55_interface_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-961-interface")
}

/// Returns the port descriptors for the ribbon controller module.
pub fn m55_ribbon_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("position-in", ComponentPortMedia::ControlVoltage),
        input_port("pressure-in", ComponentPortMedia::ControlVoltage),
        output_port("pitch-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("pressure-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("s-trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the ribbon controller module.
pub fn m55_ribbon_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("range-octaves"),
            "Range",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 4.0)),
    ]
}

/// Returns the port descriptors for the keyboard controller module.
pub fn m55_keyboard_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("key-in", ComponentPortMedia::Metadata),
        input_port("gate-in", ComponentPortMedia::Gate),
        input_port("bend-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("pitch-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("s-trigger-out", ComponentPortMedia::Gate),
        output_port("trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the keyboard controller module.
pub fn m55_keyboard_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("reference-key"),
            "Reference key",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(i64::from(System55KeyboardSettings::default().reference_key)),
        ComponentParamDescriptor::new(
            param_key("bend-depth-octaves"),
            "Bend depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 4.0, 1.0)),
    ]
}

/// Returns the port descriptors for the interface module.
pub fn m55_interface_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("s-trigger-in", ComponentPortMedia::Gate).optional(),
        input_port("voltage-gate-in", ComponentPortMedia::Gate).optional(),
        output_port("voltage-gate-out", ComponentPortMedia::Gate),
        output_port("s-trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the interface module (none).
pub fn m55_interface_params() -> Vec<ComponentParamDescriptor> {
    Vec::new()
}

fn sanitize_ribbon(settings: System55RibbonSettings) -> System55RibbonSettings {
    System55RibbonSettings {
        range_octaves: settings.range_octaves.clamp(0.0, 8.0),
        center_cv_v: settings.center_cv_v.clamp(-10.0, 10.0),
    }
}

fn sanitize_keyboard(settings: System55KeyboardSettings) -> System55KeyboardSettings {
    System55KeyboardSettings {
        reference_key: settings.reference_key.min(127),
        bend_depth_octaves: settings.bend_depth_octaves.clamp(0.0, 4.0),
    }
}

fn trace_output(id: Symbol, clock: u64, output: f32) -> ComponentTraceFrame {
    ComponentTraceFrame::new(id, ComponentBackend::Algorithmic, clock).with_output(
        trace_key("output"),
        ComponentTraceValue::Float(f64::from(output)),
    )
}

fn trace_bool_output(id: Symbol, clock: u64, output: bool) -> ComponentTraceFrame {
    ComponentTraceFrame::new(id, ComponentBackend::Algorithmic, clock)
        .with_output(trace_key("output"), ComponentTraceValue::Bool(output))
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
