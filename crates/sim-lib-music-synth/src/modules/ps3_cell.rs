//! PS-3300-style per-key voice cells and the polyphonic cell array.
//!
//! Each key of the PS-3300 has its own VCF/envelope/VCA chain rather than a
//! shared voice. [`Ps3300NoteCell`] models that per-key cell, and
//! [`Ps3300PolyArray`] assembles one cell per key into the section-wide
//! polyphonic array that renders a chord. Both are [`DiscreteComponent`]s.

use sim_kernel::Symbol;

use crate::{
    ComponentParamDescriptor, ComponentParamRange, ComponentParamUnit, ComponentPortDescriptor,
    ComponentPortDirection, ComponentPortMedia, ps3300::PS3300_KEY_COUNT,
};

mod components;
pub use components::*;

/// Fixture names for the voice-cell conformance scenarios (per-key cell chain,
/// poly-array chord cell count, poly-array gate isolation).
pub const PS3300_VOICE_CELL_FIXTURE_NAMES: [&str; 3] = [
    "ps3300-ps3-per-key-cell-vcf-envelope-vca",
    "ps3300-ps3-poly-array-chord-cell-count",
    "ps3300-ps3-poly-array-gate-isolation",
];

/// Returns the component ids for the voice-cell family: per-key cell, poly
/// array, and the companion resonator.
pub fn ps3300_voice_cell_module_ids() -> [Symbol; 3] {
    [
        ps3_per_key_cell_component_id(),
        ps3_poly_array_component_id(),
        ps3_resonator_component_id(),
    ]
}

/// Returns the voice-cell fixture names plus the two resonator scenarios.
pub fn ps3300_voice_cell_fixture_names() -> [&'static str; 5] {
    [
        PS3300_VOICE_CELL_FIXTURE_NAMES[0],
        PS3300_VOICE_CELL_FIXTURE_NAMES[1],
        PS3300_VOICE_CELL_FIXTURE_NAMES[2],
        "ps3300-ps3-resonator-peaks",
        "ps3300-ps3-resonator-formant-sweep",
    ]
}

/// Returns the qualified component id for the per-key voice cell module.
pub fn ps3_per_key_cell_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-per-key-cell")
}

/// Returns the qualified component id for the poly array module.
pub fn ps3_poly_array_component_id() -> Symbol {
    Symbol::qualified("audio-synth/module", "ps3-poly-array")
}

/// Returns the per-key cell's ports: audio, pitch CV, and gate inputs plus
/// audio, envelope, and filter outputs.
pub fn ps3_per_key_cell_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("pitch-cv-in", ComponentPortMedia::ControlVoltage),
        input_port("gate-in", ComponentPortMedia::Gate),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("envelope-out", ComponentPortMedia::ControlRate).optional(),
        output_port("filter-out", ComponentPortMedia::AudioRate).optional(),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the per-key cell's parameters: filter cutoff and resonance plus the
/// ADSR envelope times and sustain.
pub fn ps3_per_key_cell_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(param_key("cutoff-hz"), "Cutoff", ComponentParamUnit::Hertz)
            .with_range(ComponentParamRange::new(20.0, 18_000.0, 1_200.0)),
        ComponentParamDescriptor::new(
            param_key("resonance"),
            "Resonance",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.35)),
        ComponentParamDescriptor::new(param_key("attack-s"), "Attack", ComponentParamUnit::Seconds)
            .with_range(ComponentParamRange::new(0.001, 2.0, 0.004)),
        ComponentParamDescriptor::new(param_key("decay-s"), "Decay", ComponentParamUnit::Seconds)
            .with_range(ComponentParamRange::new(0.001, 4.0, 0.12)),
        ComponentParamDescriptor::new(
            param_key("sustain"),
            "Sustain",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.68)),
        ComponentParamDescriptor::new(
            param_key("release-s"),
            "Release",
            ComponentParamUnit::Seconds,
        )
        .with_range(ComponentParamRange::new(0.001, 8.0, 0.18)),
    ]
}

/// Returns the poly array's ports: audio and gate inputs plus the mixed audio
/// output.
pub fn ps3_poly_array_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        input_port("audio-in", ComponentPortMedia::AudioRate),
        input_port("gate-in", ComponentPortMedia::Gate),
        output_port("audio-out", ComponentPortMedia::AudioRate),
        output_port("trace-out", ComponentPortMedia::Trace).optional(),
    ]
}

/// Returns the poly array's parameters: section level and key count.
pub fn ps3_poly_array_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            param_key("section-level"),
            "Section level",
            ComponentParamUnit::Normalized,
        )
        .with_range(ComponentParamRange::new(0.0, 1.0, 0.75)),
        ComponentParamDescriptor::new(
            param_key("key-count"),
            "Key count",
            ComponentParamUnit::RawInteger,
        )
        .with_raw_default(PS3300_KEY_COUNT as i64),
    ]
}

fn input_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Input, 1)
}

fn output_port(name: &'static str, media: ComponentPortMedia) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(port_key(name), media, ComponentPortDirection::Output, 1)
}

fn ps3_resonator_component_id() -> Symbol {
    crate::modules::ps3_resonator::ps3_resonator_component_id()
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
