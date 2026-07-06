use sim_kernel::Symbol;

use super::{
    ComponentCapability, ComponentFactory, ComponentRegistry, ComponentRegistryCategory,
    ComponentRegistryEntry, InstrumentWrapperCategory,
};
use crate::{
    ComponentParamDescriptor, ComponentPortDescriptor, DiscreteComponentGraph, Dx7FmOperator,
    Dx7ModeledOperator, Dx7Voice, SubtractiveSynth, SynthPreset, discrete_component_graph_id,
    discrete_component_graph_ports, dx7_modeled_operator_component_id, dx7_operator_component_id,
    dx7_operator_params, dx7_operator_ports, dx7_voice_params, dx7_voice_ports,
    registry_ps3300::{ps_3300_registry_entry, ps3300_registry_entries},
    registry_system55::{system55_instrument_registry_entry, system55_registry_entries},
    subtractive_synth_component_id, subtractive_synth_params, subtractive_synth_ports,
    system700::{
        System700, System700Clock, System700Envelope, System700ExternalInput, System700Keyboard,
        System700Lfo, System700Mixer, System700Multiple, System700Noise, System700RingModulator,
        System700SampleHold, System700Sequencer, System700Vca, System700Vcf, System700Vco,
        System700VoltageProcessor, r700_clock_component_id, r700_clock_params, r700_clock_ports,
        r700_envelope_component_id, r700_envelope_params, r700_envelope_ports,
        r700_external_input_component_id, r700_external_input_params, r700_external_input_ports,
        r700_keyboard_component_id, r700_keyboard_params, r700_keyboard_ports,
        r700_lfo_component_id, r700_lfo_params, r700_lfo_ports, r700_mixer_component_id,
        r700_mixer_params, r700_mixer_ports, r700_multiple_component_id, r700_multiple_params,
        r700_multiple_ports, r700_noise_component_id, r700_noise_params, r700_noise_ports,
        r700_ring_component_id, r700_ring_params, r700_ring_ports, r700_sample_hold_component_id,
        r700_sample_hold_params, r700_sample_hold_ports, r700_sequencer_component_id,
        r700_sequencer_params, r700_sequencer_ports, r700_vca_component_id, r700_vca_params,
        r700_vca_ports, r700_vcf_component_id, r700_vcf_params, r700_vcf_ports,
        r700_vco_component_id, r700_vco_params, r700_vco_ports,
        r700_voltage_processor_component_id, r700_voltage_processor_params,
        r700_voltage_processor_ports, system700_params, system700_ports,
    },
};

/// Builds the default registry of every component this crate ships: the
/// SubtractiveSynth, the discrete graph, DX7 operators, System 700, System 55,
/// and PS-3300 modules, plus the four whole-instrument wrappers.
pub fn default_audio_synth_registry() -> ComponentRegistry {
    let mut registry = ComponentRegistry::new();
    registry
        .register(subtractive_synth_registry_entry())
        .expect("default registry ids are unique");
    registry
        .register(component_graph_registry_entry())
        .expect("default registry ids are unique");
    registry
        .register(dx7_operator_registry_entry())
        .expect("default registry ids are unique");
    registry
        .register(dx7_modeled_operator_registry_entry())
        .expect("default registry ids are unique");
    for entry in [
        r700_vco_registry_entry(),
        r700_lfo_registry_entry(),
        r700_noise_registry_entry(),
        r700_vcf_registry_entry(),
        r700_vca_registry_entry(),
        r700_ring_registry_entry(),
        r700_envelope_registry_entry(),
        r700_sample_hold_registry_entry(),
        r700_voltage_processor_registry_entry(),
        r700_mixer_registry_entry(),
        r700_multiple_registry_entry(),
        r700_external_input_registry_entry(),
        r700_keyboard_registry_entry(),
        r700_clock_registry_entry(),
        r700_sequencer_registry_entry(),
    ] {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    for entry in system55_registry_entries() {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    for entry in ps3300_registry_entries() {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    for entry in [
        dx7_registry_entry(),
        system_700_registry_entry(),
        system55_instrument_registry_entry(),
        ps_3300_registry_entry(),
    ] {
        registry
            .register(entry)
            .expect("default registry ids are unique");
    }
    registry
}

/// Returns the registry entry for the built-in subtractive polysynth.
pub fn subtractive_synth_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        subtractive_synth_component_id(),
        "SubtractiveSynth",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::FixedPolysynth,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(subtractive_synth_ports())
    .with_params(subtractive_synth_params())
    .with_factory(|| Box::new(SubtractiveSynth::new(SynthPreset::default())))
}

/// Returns the registry entry for the user-built discrete component graph.
pub fn component_graph_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        discrete_component_graph_id(),
        "DiscreteComponentGraph",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::CustomGraph,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(discrete_component_graph_ports())
    .with_factory(|| {
        Box::new(DiscreteComponentGraph::new(Symbol::qualified(
            "audio-synth",
            "custom-graph",
        )))
    })
}

/// Returns the registry entry for a single algorithmic DX7 FM operator.
pub fn dx7_operator_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        dx7_operator_component_id(),
        "DX7 Operator",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::Dx7,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(dx7_operator_ports())
    .with_params(dx7_operator_params())
    .with_factory(|| Box::new(Dx7FmOperator::default()))
}

/// Returns the registry entry for the analog-modeled DX7 FM operator.
pub fn dx7_modeled_operator_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        dx7_modeled_operator_component_id(),
        "DX7 Modeled Operator",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::Dx7,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(dx7_operator_ports())
    .with_params(dx7_operator_params())
    .with_factory(|| Box::new(Dx7ModeledOperator::default()))
}

/// Returns the component id of the whole DX7 instrument.
pub fn dx7_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "dx7")
}

/// Returns the component id of the Roland System 700 instrument.
pub fn system_700_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "roland-system-700")
}

/// Returns the component id of the Moog System 55 instrument.
pub fn system_55_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "moog-system-55")
}

/// Returns the component id of the Korg PS-3300 instrument.
pub fn ps_3300_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "korg-ps-3300")
}

/// Returns the registry entry for the whole DX7 voice instrument.
pub fn dx7_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        dx7_component_id(),
        "DX7",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::Dx7,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(dx7_voice_ports())
    .with_params(dx7_voice_params())
    .with_factory(|| Box::new(Dx7Voice::default()))
}

/// Returns the registry entry for the System 700 VCO module.
pub fn r700_vco_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_vco_component_id(),
        "R700 VCO",
        r700_vco_ports(),
        r700_vco_params(),
        || Box::new(System700Vco::default()),
    )
}

/// Returns the registry entry for the System 700 LFO module.
pub fn r700_lfo_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_lfo_component_id(),
        "R700 LFO",
        r700_lfo_ports(),
        r700_lfo_params(),
        || Box::new(System700Lfo::default()),
    )
}

/// Returns the registry entry for the System 700 noise source module.
pub fn r700_noise_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_noise_component_id(),
        "R700 Noise",
        r700_noise_ports(),
        r700_noise_params(),
        || Box::new(System700Noise::default()),
    )
}

/// Returns the registry entry for the System 700 VCF module.
pub fn r700_vcf_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_vcf_component_id(),
        "R700 VCF",
        r700_vcf_ports(),
        r700_vcf_params(),
        || Box::new(System700Vcf::default()),
    )
}

/// Returns the registry entry for the System 700 VCA module.
pub fn r700_vca_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_vca_component_id(),
        "R700 VCA",
        r700_vca_ports(),
        r700_vca_params(),
        || Box::new(System700Vca::default()),
    )
}

/// Returns the registry entry for the System 700 ring modulator module.
pub fn r700_ring_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_ring_component_id(),
        "R700 Ring",
        r700_ring_ports(),
        r700_ring_params(),
        || Box::new(System700RingModulator::default()),
    )
}

/// Returns the registry entry for the System 700 envelope generator module.
pub fn r700_envelope_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_envelope_component_id(),
        "R700 Envelope",
        r700_envelope_ports(),
        r700_envelope_params(),
        || Box::new(System700Envelope::default()),
    )
}

/// Returns the registry entry for the System 700 sample-and-hold module.
pub fn r700_sample_hold_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_sample_hold_component_id(),
        "R700 Sample and Hold",
        r700_sample_hold_ports(),
        r700_sample_hold_params(),
        || Box::new(System700SampleHold::default()),
    )
}

/// Returns the registry entry for the System 700 voltage processor module.
pub fn r700_voltage_processor_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_voltage_processor_component_id(),
        "R700 Voltage Processor",
        r700_voltage_processor_ports(),
        r700_voltage_processor_params(),
        || Box::new(System700VoltageProcessor::default()),
    )
}

/// Returns the registry entry for the System 700 mixer module.
pub fn r700_mixer_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_mixer_component_id(),
        "R700 Mixer",
        r700_mixer_ports(),
        r700_mixer_params(),
        || Box::new(System700Mixer::default()),
    )
}

/// Returns the registry entry for the System 700 multiple (signal splitter) module.
pub fn r700_multiple_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_multiple_component_id(),
        "R700 Multiple",
        r700_multiple_ports(),
        r700_multiple_params(),
        || Box::new(System700Multiple::default()),
    )
}

/// Returns the registry entry for the System 700 external input module.
pub fn r700_external_input_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_external_input_component_id(),
        "R700 External Input",
        r700_external_input_ports(),
        r700_external_input_params(),
        || Box::new(System700ExternalInput::default()),
    )
}

/// Returns the registry entry for the System 700 keyboard controller module.
pub fn r700_keyboard_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_keyboard_component_id(),
        "R700 Keyboard",
        r700_keyboard_ports(),
        r700_keyboard_params(),
        || Box::new(System700Keyboard::default()),
    )
}

/// Returns the registry entry for the System 700 clock module.
pub fn r700_clock_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_clock_component_id(),
        "R700 Clock",
        r700_clock_ports(),
        r700_clock_params(),
        || Box::new(System700Clock::default()),
    )
}

/// Returns the registry entry for the System 700 sequencer module.
pub fn r700_sequencer_registry_entry() -> ComponentRegistryEntry {
    exact_modular_entry(
        r700_sequencer_component_id(),
        "R700 Sequencer",
        r700_sequencer_ports(),
        r700_sequencer_params(),
        || Box::new(System700Sequencer::default()),
    )
}

fn exact_modular_entry(
    id: Symbol,
    label: &'static str,
    ports: Vec<ComponentPortDescriptor>,
    params: Vec<ComponentParamDescriptor>,
    factory: ComponentFactory,
) -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        id,
        label,
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::ModularAnalog,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(ports)
    .with_params(params)
    .with_factory(factory)
}

fn system_700_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        system_700_component_id(),
        "Roland System 700",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::ModularAnalog,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(system700_ports())
    .with_params(system700_params())
    .with_factory(|| Box::new(System700::default()))
}
