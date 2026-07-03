use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Configuration for a [`System55VcoDriver`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55VcoDriverSettings {
    /// Coarse transposition added to the pitch control voltage, in octaves.
    pub transpose_octaves: f32,
    /// Fine tuning added to the pitch control voltage, in semitones.
    pub fine_tune_semitones: f32,
    /// Depth applied to the modulation control-voltage input, in octaves.
    pub modulation_depth_octaves: f32,
}

impl Default for System55VcoDriverSettings {
    fn default() -> Self {
        Self {
            transpose_octaves: 0.0,
            fine_tune_semitones: 0.0,
            modulation_depth_octaves: 1.0,
        }
    }
}

/// One frame of control voltages and sync state produced by a [`System55VcoDriver`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55VcoDriverFrame {
    /// Pitch control voltage in volts-per-octave, including transpose and fine tune.
    pub pitch_cv_v: f32,
    /// Modulation control voltage in volts, after depth scaling.
    pub modulation_cv_v: f32,
    /// Whether the sync input is currently high.
    pub sync_high: bool,
    /// Whether a rising sync edge occurred on this frame.
    pub sync_triggered: bool,
}

impl System55VcoDriverFrame {
    /// Returns the combined pitch-plus-modulation control voltage.
    pub fn tracking_cv_v(self) -> f32 {
        self.pitch_cv_v + self.modulation_cv_v
    }
}

/// Conditions keyboard and modulation control voltages and tracks sync edges for a VCO.
#[derive(Clone, Debug, PartialEq)]
pub struct System55VcoDriver {
    settings: System55VcoDriverSettings,
    last_sync_high: bool,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System55VcoDriver {
    /// Creates a driver from `settings`, clamping them into valid ranges.
    pub fn new(settings: System55VcoDriverSettings) -> Self {
        Self {
            settings: sanitize(settings),
            last_sync_high: false,
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the sanitized settings in effect.
    pub fn settings(&self) -> System55VcoDriverSettings {
        self.settings
    }

    /// Produces the next driver frame from keyboard CV, modulation CV, and sync level.
    pub fn next_frame(
        &mut self,
        keyboard_cv_v: f32,
        modulation_cv_v: f32,
        sync_high: bool,
    ) -> System55VcoDriverFrame {
        let frame = System55VcoDriverFrame {
            pitch_cv_v: keyboard_cv_v
                + self.settings.transpose_octaves
                + self.settings.fine_tune_semitones / 12.0,
            modulation_cv_v: modulation_cv_v * self.settings.modulation_depth_octaves,
            sync_high,
            sync_triggered: sync_high && !self.last_sync_high,
        };
        self.last_sync_high = sync_high;
        self.last_trace = Some(self.trace_frame(frame));
        self.clock = self.clock.saturating_add(1);
        frame
    }

    fn trace_frame(&self, frame: System55VcoDriverFrame) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            m55_vco_driver_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("pitch-cv-v"),
            ComponentTraceValue::Float(f64::from(frame.pitch_cv_v)),
        )
        .with_state(
            trace_key("modulation-cv-v"),
            ComponentTraceValue::Float(f64::from(frame.modulation_cv_v)),
        )
        .with_state(
            trace_key("sync-high"),
            ComponentTraceValue::Bool(frame.sync_high),
        )
        .with_state(
            trace_key("sync-triggered"),
            ComponentTraceValue::Bool(frame.sync_triggered),
        )
    }
}

impl Default for System55VcoDriver {
    fn default() -> Self {
        Self::new(System55VcoDriverSettings::default())
    }
}

impl DiscreteComponent for System55VcoDriver {
    fn component_id(&self) -> Symbol {
        m55_vco_driver_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        m55_vco_driver_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        m55_vco_driver_params()
    }

    fn reset(&mut self) {
        self.last_sync_high = false;
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let output = self.next_frame(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
                input(block.in_audio, 2, frame) > 0.5,
            );
            write_output(block.out_audio, 0, frame, output.pitch_cv_v);
            write_output(block.out_audio, 1, frame, output.modulation_cv_v);
            write_output(block.out_audio, 2, frame, f32::from(output.sync_high));
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(
            m55_vco_driver_component_id(),
            ComponentBackend::Algorithmic,
            true,
        )
        .with_field(
            inspect_key("transpose-octaves"),
            self.settings.transpose_octaves.to_string(),
        )
        .with_field(
            inspect_key("modulation-depth-octaves"),
            self.settings.modulation_depth_octaves.to_string(),
        )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 55 VCO driver module.
pub fn m55_vco_driver_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-921a-oscillator-driver")
}

/// Returns the port descriptors for the System 55 VCO driver module.
pub fn m55_vco_driver_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("keyboard-cv-in", ComponentPortMedia::ControlVoltage),
        input_port("modulation-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        input_port("sync-in", ComponentPortMedia::Gate).optional(),
        output_port("pitch-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("modulation-cv-out", ComponentPortMedia::ControlVoltage).optional(),
        output_port("sync-out", ComponentPortMedia::Gate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 55 VCO driver module.
pub fn m55_vco_driver_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("transpose-octaves"),
            "Transpose",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-5.0, 5.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("fine-tune-semitones"),
            "Fine tune",
            ComponentParamUnit::Semitones,
        )
        .with_range(ComponentParamRange::new(-12.0, 12.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("modulation-depth-octaves"),
            "Modulation depth",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
    ]
}

fn sanitize(settings: System55VcoDriverSettings) -> System55VcoDriverSettings {
    System55VcoDriverSettings {
        transpose_octaves: settings.transpose_octaves.clamp(-5.0, 5.0),
        fine_tune_semitones: settings.fine_tune_semitones.clamp(-12.0, 12.0),
        modulation_depth_octaves: settings.modulation_depth_octaves.clamp(0.0, 8.0),
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
