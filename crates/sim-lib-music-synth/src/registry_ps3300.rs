use crate::{
    ComponentParamDescriptor, ComponentPortDescriptor, DiscreteComponent,
    modules::{
        ps3_keyboard::{
            Ps3300KeyboardController, ps3_keyboard_component_id, ps3_keyboard_params,
            ps3_keyboard_ports,
        },
        ps3_matrix::{
            Ps3300PinMatrix, ps3_pin_matrix_component_id, ps3_pin_matrix_params,
            ps3_pin_matrix_ports,
        },
        ps3_modulation::{
            Ps3300ExternalProcessor, Ps3300ModulationGenerator, Ps3300SampleHold,
            ps3_external_processor_component_id, ps3_external_processor_params,
            ps3_external_processor_ports, ps3_modulation_generator_component_id,
            ps3_modulation_generator_params, ps3_modulation_generator_ports,
            ps3_sample_hold_component_id, ps3_sample_hold_params, ps3_sample_hold_ports,
        },
        ps3_section::{
            Ps3300SectionGenerator, Ps3300ThreeSectionSummer, ps3_output_mixer_component_id,
            ps3_output_mixer_params, ps3_output_mixer_ports, ps3_section_generator_component_id,
            ps3_section_generator_params, ps3_section_generator_ports,
        },
    },
    ps3300::{
        Ps3300Noise, Ps3300NoteCell, Ps3300PolyArray, Ps3300ToneSource, Ps3300TripleResonator,
        ps3_noise_component_id, ps3_noise_params, ps3_noise_ports, ps3_per_key_cell_component_id,
        ps3_per_key_cell_params, ps3_per_key_cell_ports, ps3_poly_array_component_id,
        ps3_poly_array_params, ps3_poly_array_ports, ps3_resonator_component_id,
        ps3_resonator_params, ps3_resonator_ports, ps3_tonegen_component_id, ps3_tonegen_params,
        ps3_tonegen_ports,
    },
    ps3300_params, ps3300_ports,
    ps3300_wrapper::Ps3300,
    registry::{
        ComponentCapability, ComponentRegistryCategory, ComponentRegistryEntry,
        InstrumentWrapperCategory, ps_3300_component_id,
    },
};

pub(crate) fn ps3300_registry_entries() -> Vec<ComponentRegistryEntry> {
    vec![
        exact_ps3300_entry(
            ps3_tonegen_component_id(),
            "PS-3300 Tone Generator",
            ps3_tonegen_ports(),
            ps3_tonegen_params(),
            || Box::new(Ps3300ToneSource::default()),
        ),
        exact_ps3300_entry(
            ps3_noise_component_id(),
            "PS-3300 Noise Source",
            ps3_noise_ports(),
            ps3_noise_params(),
            || Box::new(Ps3300Noise::default()),
        ),
        exact_ps3300_entry(
            ps3_per_key_cell_component_id(),
            "PS-3300 Per-Key Cell",
            ps3_per_key_cell_ports(),
            ps3_per_key_cell_params(),
            || Box::new(Ps3300NoteCell::default()),
        ),
        exact_ps3300_entry(
            ps3_poly_array_component_id(),
            "PS-3300 Polyphonic Array",
            ps3_poly_array_ports(),
            ps3_poly_array_params(),
            || Box::new(Ps3300PolyArray::default()),
        ),
        exact_ps3300_entry(
            ps3_resonator_component_id(),
            "PS-3300 Resonator Bank",
            ps3_resonator_ports(),
            ps3_resonator_params(),
            || Box::new(Ps3300TripleResonator::default()),
        ),
        exact_ps3300_entry(
            ps3_modulation_generator_component_id(),
            "PS-3300 Modulation Generator",
            ps3_modulation_generator_ports(),
            ps3_modulation_generator_params(),
            || Box::new(Ps3300ModulationGenerator::default()),
        ),
        exact_ps3300_entry(
            ps3_sample_hold_component_id(),
            "PS-3300 Sample and Hold",
            ps3_sample_hold_ports(),
            ps3_sample_hold_params(),
            || Box::new(Ps3300SampleHold::default()),
        ),
        exact_ps3300_entry(
            ps3_external_processor_component_id(),
            "PS-3300 External Processor",
            ps3_external_processor_ports(),
            ps3_external_processor_params(),
            || Box::new(Ps3300ExternalProcessor::default()),
        ),
        exact_ps3300_entry(
            ps3_keyboard_component_id(),
            "PS-3300 Keyboard Controller",
            ps3_keyboard_ports(),
            ps3_keyboard_params(),
            || Box::new(Ps3300KeyboardController::default()),
        ),
        exact_ps3300_entry(
            ps3_pin_matrix_component_id(),
            "PS-3300 Pin Matrix",
            ps3_pin_matrix_ports(),
            ps3_pin_matrix_params(),
            || Box::new(Ps3300PinMatrix::default()),
        ),
        exact_ps3300_entry(
            ps3_section_generator_component_id(),
            "PS-3300 Section Generator",
            ps3_section_generator_ports(),
            ps3_section_generator_params(),
            || Box::new(Ps3300SectionGenerator::default()),
        ),
        exact_ps3300_entry(
            ps3_output_mixer_component_id(),
            "PS-3300 Output Mixer",
            ps3_output_mixer_ports(),
            ps3_output_mixer_params(),
            || Box::new(Ps3300ThreeSectionSummer::default()),
        ),
    ]
}

pub(crate) fn ps_3300_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        ps_3300_component_id(),
        "Korg PS-3300",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::FixedPolysynth,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_capability(ComponentCapability::Traceable)
    .with_ports(ps3300_ports())
    .with_params(ps3300_params())
    .with_factory(|| Box::new(Ps3300::default()))
}

fn exact_ps3300_entry(
    id: sim_kernel::Symbol,
    label: &'static str,
    ports: Vec<ComponentPortDescriptor>,
    params: Vec<ComponentParamDescriptor>,
    factory: fn() -> Box<dyn DiscreteComponent>,
) -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        id,
        label,
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::FixedPolysynth,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(ports)
    .with_params(params)
    .with_factory(factory)
}
