use sim_kernel::Symbol;
use sim_lib_audio_graph_core::ProcessBlock;

use crate::{
    ComponentBackend, ComponentInspection, ComponentParamDescriptor, ComponentParamRange,
    ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection, ComponentPortMedia,
    ComponentPrepareConfig, ComponentTraceFrame, ComponentTraceValue, DiscreteComponent,
};

/// Control-voltage response curve applied by the System 700 VCA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System700VcaResponse {
    /// Gain tracks the control voltage directly (unity slope).
    Linear,
    /// Gain follows the squared control voltage for an exponential taper.
    Exponential,
    /// Linear control gain followed by tanh saturation of the output.
    Saturated,
}

impl System700VcaResponse {
    /// Returns the stable lowercase identifier for this response curve.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Exponential => "exponential",
            Self::Saturated => "saturated",
        }
    }

    /// Returns the qualified [`Symbol`] naming this response curve.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/r700-vca-response", self.as_str())
    }
}

/// Configuration for a [`System700Vca`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System700VcaSettings {
    /// Control-voltage response curve.
    pub response: System700VcaResponse,
    /// Maximum gain applied at full control voltage.
    pub gain: f32,
    /// Drive amount feeding the tanh stage in [`System700VcaResponse::Saturated`] mode.
    pub saturation_drive: f32,
}

impl Default for System700VcaSettings {
    fn default() -> Self {
        Self {
            response: System700VcaResponse::Linear,
            gain: 1.0,
            saturation_drive: 2.0,
        }
    }
}

/// Voltage-controlled amplifier whose gain is driven by a control-voltage input.
#[derive(Clone, Debug, PartialEq)]
pub struct System700Vca {
    settings: System700VcaSettings,
    clock: u64,
    last_trace: Option<ComponentTraceFrame>,
}

impl System700Vca {
    /// Creates a VCA from `settings`, clamping them into valid ranges.
    pub fn new(settings: System700VcaSettings) -> Self {
        Self {
            settings: sanitize(settings),
            clock: 0,
            last_trace: None,
        }
    }

    /// Returns the sanitized settings in effect.
    pub fn settings(&self) -> System700VcaSettings {
        self.settings
    }

    /// Maps a control voltage in `0.0..=1.0` to the gain applied to the input.
    pub fn gain_for_cv(&self, cv: f32) -> f32 {
        let cv = cv.clamp(0.0, 1.0);
        let control = match self.settings.response {
            System700VcaResponse::Linear | System700VcaResponse::Saturated => cv,
            System700VcaResponse::Exponential => cv * cv,
        };
        control * self.settings.gain
    }

    /// Amplifies one `input` sample by the gain derived from `cv`, recording a trace.
    pub fn next_sample(&mut self, input: f32, cv: f32) -> f32 {
        let gain = self.gain_for_cv(cv);
        let raw = input * gain;
        let output = match self.settings.response {
            System700VcaResponse::Saturated => saturate(raw, self.settings.saturation_drive),
            System700VcaResponse::Linear | System700VcaResponse::Exponential => raw,
        };
        self.last_trace = Some(self.trace_frame(gain, output));
        self.clock = self.clock.saturating_add(1);
        output
    }

    fn trace_frame(&self, gain: f32, output: f32) -> ComponentTraceFrame {
        ComponentTraceFrame::new(
            r700_vca_component_id(),
            ComponentBackend::Algorithmic,
            self.clock,
        )
        .with_state(
            trace_key("gain"),
            ComponentTraceValue::Float(f64::from(gain)),
        )
        .with_output(
            trace_key("output"),
            ComponentTraceValue::Float(f64::from(output)),
        )
    }
}

impl Default for System700Vca {
    fn default() -> Self {
        Self::new(System700VcaSettings::default())
    }
}

impl DiscreteComponent for System700Vca {
    fn component_id(&self) -> Symbol {
        r700_vca_component_id()
    }

    fn backend(&self) -> ComponentBackend {
        ComponentBackend::Algorithmic
    }

    fn ports(&self) -> Vec<ComponentPortDescriptor> {
        r700_vca_ports()
    }

    fn params(&self) -> Vec<ComponentParamDescriptor> {
        r700_vca_params()
    }

    fn reset(&mut self) {
        self.clock = 0;
        self.last_trace = None;
    }

    fn prepare(&mut self, _config: ComponentPrepareConfig) {}

    fn render(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for frame in 0..frames {
            let sample = self.next_sample(
                input(block.in_audio, 0, frame),
                input(block.in_audio, 1, frame),
            );
            for channel in &mut *block.out_audio {
                channel[frame] = sample;
            }
        }
    }

    fn inspect(&self) -> ComponentInspection {
        ComponentInspection::new(r700_vca_component_id(), ComponentBackend::Algorithmic, true)
            .with_field(
                inspect_key("response"),
                self.settings.response.as_str().to_owned(),
            )
    }

    fn trace(&self) -> Option<ComponentTraceFrame> {
        self.last_trace.clone()
    }
}

/// Returns the component id of the System 700 VCA module.
pub fn r700_vca_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "r700-vca")
}

/// Returns the port descriptors for the System 700 VCA module.
pub fn r700_vca_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("gain-cv-in", ComponentPortMedia::ControlVoltage),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 VCA module.
pub fn r700_vca_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("response"),
            "Response",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                System700VcaResponse::Linear.symbol(),
                System700VcaResponse::Exponential.symbol(),
                System700VcaResponse::Saturated.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(param_key("gain"), "Gain", ComponentParamUnit::Unitless)
            .with_range(ComponentParamRange::new(0.0, 4.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("saturation-drive"),
            "Saturation drive",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.5, 12.0, 2.0)),
    ]
}

fn sanitize(settings: System700VcaSettings) -> System700VcaSettings {
    System700VcaSettings {
        response: settings.response,
        gain: settings.gain.clamp(0.0, 4.0),
        saturation_drive: settings.saturation_drive.clamp(0.5, 12.0),
    }
}

fn saturate(input: f32, drive: f32) -> f32 {
    let scale = drive.tanh().max(0.001);
    ((input * drive).tanh() / scale).clamp(-1.0, 1.0)
}

fn input(channels: &[&[f32]], channel: usize, frame: usize) -> f32 {
    channels
        .get(channel)
        .and_then(|samples| samples.get(frame))
        .copied()
        .unwrap_or(0.0)
}

fn input_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Input, 1)
}

fn output_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Output, 1)
}

fn port_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-port", name)
}

fn param_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-param", name)
}

fn inspect_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-inspect", name)
}

fn trace_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/r700-trace", name)
}
