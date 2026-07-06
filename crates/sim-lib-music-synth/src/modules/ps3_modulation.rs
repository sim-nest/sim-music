//! PS-3300-style modulation sources and signal conditioning.
//!
//! Provides the control-rail building blocks of the PS-3300: a low-frequency
//! modulation generator, a sample-and-hold, and an external-signal processor
//! that gains, biases, and envelope-follows an outside input into control
//! voltages. Each is a [`DiscreteComponent`] and feeds the tone and cell stages.

use sim_kernel::Symbol;

use crate::{
    ComponentParamDescriptor, ComponentParamRange, ComponentParamUnit, ComponentPortDescriptor,
    ComponentPortDirection, ComponentPortMedia,
};

mod components;
pub use components::*;

/// Fixture names for the modulation conformance scenarios (generator shapes,
/// sample-and-hold edge capture, external-processor tracking).
pub const PS3300_MODULATION_FIXTURE_NAMES: [&str; 3] = [
    "ps3300-ps3-modulation-generator-shapes",
    "ps3300-ps3-sample-hold-edge-capture",
    "ps3300-ps3-external-processor-tracking",
];

/// Returns the component ids for the modulation family: generator, sample-hold,
/// and external processor.
pub fn ps3300_modulation_module_ids() -> [Symbol; 3] {
    [
        ps3_modulation_generator_component_id(),
        ps3_sample_hold_component_id(),
        ps3_external_processor_component_id(),
    ]
}

/// Returns the fixture names for the modulation conformance scenarios.
pub fn ps3300_modulation_fixture_names() -> [&'static str; 3] {
    PS3300_MODULATION_FIXTURE_NAMES
}

/// Returns the qualified component id for the modulation generator module.
pub fn ps3_modulation_generator_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-modulation-generator")
}

/// Returns the qualified component id for the sample-and-hold module.
pub fn ps3_sample_hold_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-sample-hold")
}

/// Returns the qualified component id for the external processor module.
pub fn ps3_external_processor_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-external-processor")
}

/// Returns the modulation generator's ports: rate CV input plus bipolar and
/// unipolar CV outputs.
pub fn ps3_modulation_generator_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("rate-cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("unipolar-out", ComponentPortMedia::ControlVoltage).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the modulation generator's parameters: waveform, rate, and depth.
pub fn ps3_modulation_generator_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("waveform"),
            "Waveform",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                Ps3300ModulationWaveform::Sine.symbol(),
                Ps3300ModulationWaveform::Triangle.symbol(),
                Ps3300ModulationWaveform::Saw.symbol(),
                Ps3300ModulationWaveform::Square.symbol(),
            ],
            0,
        ),
        ComponentParamDescriptor::new(param_key("rate-hz"), "Rate", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(0.01, 100.0, 5.0)),
        ComponentParamDescriptor::new(param_key("depth"), "Depth", ComponentParamUnit::Normalized)
            .with_range(ComponentParamRange::new(0.0, 1.0, 1.0)),
    ]
}

/// Returns the sample-and-hold's ports: signal and trigger inputs, held CV
/// output, and a capture gate.
pub fn ps3_sample_hold_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("signal-in", ComponentPortMedia::ControlVoltage),
        input_port("trigger-in", ComponentPortMedia::Gate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("capture-out", ComponentPortMedia::Gate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the sample-and-hold's parameters: initial value and trigger
/// threshold.
pub fn ps3_sample_hold_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("initial-value"),
            "Initial value",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 0.0)),
        ComponentParamDescriptor::new(
            param_key("trigger-threshold-v"),
            "Trigger threshold",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 10.0, 0.5)),
    ]
}

/// Returns the external processor's ports: audio and CV inputs and the audio,
/// CV, gate, and follower outputs.
pub fn ps3_external_processor_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("cv-in", ComponentPortMedia::ControlVoltage).optional(),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("cv-out", ComponentPortMedia::ControlVoltage),
        output_port("gate-out", ComponentPortMedia::Gate),
        output_port("follower-out", ComponentPortMedia::ControlVoltage).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the external processor's parameters: audio gain, CV gain, and gate
/// threshold.
pub fn ps3_external_processor_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("audio-gain"),
            "Audio gain",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("cv-gain"),
            "CV gain",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(0.0, 8.0, 1.0)),
        ComponentParamDescriptor::new(
            param_key("gate-threshold-v"),
            "Gate threshold",
            ComponentParamUnit::Unitless,
        )
        .with_range(ComponentParamRange::new(-10.0, 10.0, 1.0)),
    ]
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
