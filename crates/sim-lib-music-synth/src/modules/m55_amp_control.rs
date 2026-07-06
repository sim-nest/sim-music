use sim_kernel::Symbol;

use crate::{
    ComponentParamDescriptor, ComponentParamRange, ComponentParamUnit, ComponentPortDescriptor,
    ComponentPortDirection, ComponentPortMedia,
};

mod components;
pub use components::*;

/// Returns the qualified module id for the M55 902 VCA.
pub fn m55_vca_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-902-vca")
}

/// Returns the qualified module id for the M55 911 envelope generator.
pub fn m55_envelope_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-911-envelope-generator")
}

/// Returns the qualified module id for the M55 911A dual trigger delay.
pub fn m55_trigger_delay_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "m55-911a-dual-trigger-delay")
}

/// Returns the port descriptors for the M55 902 VCA.
pub fn m55_vca_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("gain-cv-in", ComponentPortMedia::ControlVoltage),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the M55 902 VCA.
pub fn m55_vca_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("response"),
            "Response",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(
            vec![
                System55VcaResponse::Linear.symbol(),
                System55VcaResponse::Exponential.symbol(),
                System55VcaResponse::Saturated.symbol(),
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

/// Returns the port descriptors for the M55 911 envelope generator.
pub fn m55_envelope_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("s-trigger-in", ComponentPortMedia::Gate),
        output_port("envelope-cv-out", ComponentPortMedia::ControlVoltage),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the M55 911 envelope generator.
pub fn m55_envelope_params() -> Vec<ComponentParamDescriptor> {
    vec![
        time_param("attack-s", "Attack", 0.01),
        time_param("decay-s", "Decay", 0.1),
        ComponentParamDescriptor::new(
            param_key("sustain-level"),
            "Sustain",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.7)),
        time_param("release-s", "Release", 0.2),
    ]
}

/// Returns the port descriptors for the M55 911A dual trigger delay.
pub fn m55_trigger_delay_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("s-trigger-in", ComponentPortMedia::Gate),
        output_port("s-trigger-out", ComponentPortMedia::Gate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the parameter descriptors for the M55 911A dual trigger delay.
pub fn m55_trigger_delay_params() -> Vec<ComponentParamDescriptor> {
    vec![
        time_param("delay-s", "Delay", 0.05),
        time_param("pulse-s", "Pulse", 0.01),
    ]
}

fn time_param(name: &'static str, label: &'static str, default: f64) -> ComponentParamDescriptor {
    ComponentParamDescriptor::new(param_key(name), label, ComponentParamUnit::Seconds)
        .with_range(ComponentParamRange::new(0.0, 20.0, default))
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
