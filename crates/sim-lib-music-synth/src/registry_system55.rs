use crate::{
    ComponentParamDescriptor, ComponentPortDescriptor, DiscreteComponent,
    registry::{
        ComponentCapability, ComponentRegistryCategory, ComponentRegistryEntry,
        InstrumentWrapperCategory,
    },
    system55::{
        System55Attenuator, System55Envelope, System55EnvelopeFollower, System55FilterCoupler,
        System55FixedFilterBank, System55FrequencyShifter, System55HighPassFilter,
        System55Interface, System55Keyboard, System55LadderLpf, System55Mixer,
        System55ModuleDescriptor, System55Multiple, System55Noise, System55Ribbon,
        System55RingModulator, System55SampleHold, System55Sequencer, System55TriggerDelay,
        System55Vca, System55Vco, System55VcoDriver, m55_attenuator_component_id,
        m55_attenuator_params, m55_attenuator_ports, m55_coupler_component_id, m55_coupler_params,
        m55_coupler_ports, m55_env_follower_component_id, m55_env_follower_params,
        m55_env_follower_ports, m55_envelope_component_id, m55_envelope_params, m55_envelope_ports,
        m55_fixed_filter_bank_component_id, m55_fixed_filter_bank_params,
        m55_fixed_filter_bank_ports, m55_frequency_shifter_component_id,
        m55_frequency_shifter_params, m55_frequency_shifter_ports, m55_hpf_component_id,
        m55_hpf_params, m55_hpf_ports, m55_interface_component_id, m55_interface_params,
        m55_interface_ports, m55_keyboard_component_id, m55_keyboard_params, m55_keyboard_ports,
        m55_ladder_lpf_component_id, m55_ladder_lpf_params, m55_ladder_lpf_ports,
        m55_mixer_component_id, m55_mixer_params, m55_mixer_ports, m55_multiple_component_id,
        m55_multiple_params, m55_multiple_ports, m55_noise_component_id, m55_noise_params,
        m55_noise_ports, m55_ribbon_component_id, m55_ribbon_params, m55_ribbon_ports,
        m55_ring_component_id, m55_ring_params, m55_ring_ports, m55_sample_hold_component_id,
        m55_sample_hold_params, m55_sample_hold_ports, m55_sequencer_component_id,
        m55_sequencer_params, m55_sequencer_ports, m55_trigger_delay_component_id,
        m55_trigger_delay_params, m55_trigger_delay_ports, m55_vca_component_id, m55_vca_params,
        m55_vca_ports, m55_vco_component_id, m55_vco_driver_component_id, m55_vco_driver_params,
        m55_vco_driver_ports, m55_vco_params, m55_vco_ports, system55_module_descriptors,
    },
    system55_patch::{system55_component_id, system55_params, system55_ports},
    system55_wrapper::System55,
};

pub(crate) fn system55_registry_entries() -> Vec<ComponentRegistryEntry> {
    system55_module_descriptors()
        .into_iter()
        .map(system55_registry_entry)
        .collect()
}

pub(crate) fn system55_instrument_registry_entry() -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        system55_component_id(),
        "Moog System 55",
        ComponentRegistryCategory::Exact,
        InstrumentWrapperCategory::ModularAnalog,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(system55_ports())
    .with_params(system55_params())
    .with_factory(|| Box::new(System55::default()))
}

fn system55_registry_entry(descriptor: System55ModuleDescriptor) -> ComponentRegistryEntry {
    match descriptor.id_name {
        "m55-921a-oscillator-driver" => exact_system55_entry(
            m55_vco_driver_component_id(),
            descriptor.label,
            m55_vco_driver_ports(),
            m55_vco_driver_params(),
            || Box::new(System55VcoDriver::default()),
        ),
        "m55-921b-oscillator" => exact_system55_entry(
            m55_vco_component_id(),
            descriptor.label,
            m55_vco_ports(),
            m55_vco_params(),
            || Box::new(System55Vco::default()),
        ),
        "m55-923-noise-filter" => exact_system55_entry(
            m55_noise_component_id(),
            descriptor.label,
            m55_noise_ports(),
            m55_noise_params(),
            || Box::new(System55Noise::default()),
        ),
        "m55-904a-low-pass-filter" => exact_system55_entry(
            m55_ladder_lpf_component_id(),
            descriptor.label,
            m55_ladder_lpf_ports(),
            m55_ladder_lpf_params(),
            || Box::new(System55LadderLpf::default()),
        ),
        "m55-904b-high-pass-filter" => exact_system55_entry(
            m55_hpf_component_id(),
            descriptor.label,
            m55_hpf_ports(),
            m55_hpf_params(),
            || Box::new(System55HighPassFilter::default()),
        ),
        "m55-904c-filter-coupler" => exact_system55_entry(
            m55_coupler_component_id(),
            descriptor.label,
            m55_coupler_ports(),
            m55_coupler_params(),
            || Box::new(System55FilterCoupler::default()),
        ),
        "m55-907-fixed-filter-bank" => exact_system55_entry(
            m55_fixed_filter_bank_component_id(),
            descriptor.label,
            m55_fixed_filter_bank_ports(),
            m55_fixed_filter_bank_params(),
            || Box::new(System55FixedFilterBank::default()),
        ),
        "m55-902-vca" => exact_system55_entry(
            m55_vca_component_id(),
            descriptor.label,
            m55_vca_ports(),
            m55_vca_params(),
            || Box::new(System55Vca::default()),
        ),
        "m55-911-envelope-generator" => exact_system55_entry(
            m55_envelope_component_id(),
            descriptor.label,
            m55_envelope_ports(),
            m55_envelope_params(),
            || Box::new(System55Envelope::default()),
        ),
        "m55-911a-dual-trigger-delay" => exact_system55_entry(
            m55_trigger_delay_component_id(),
            descriptor.label,
            m55_trigger_delay_ports(),
            m55_trigger_delay_params(),
            || Box::new(System55TriggerDelay::default()),
        ),
        "m55-912-envelope-follower" => exact_system55_entry(
            m55_env_follower_component_id(),
            descriptor.label,
            m55_env_follower_ports(),
            m55_env_follower_params(),
            || Box::new(System55EnvelopeFollower::default()),
        ),
        "m55-1630-frequency-shifter" => exact_system55_entry(
            m55_frequency_shifter_component_id(),
            descriptor.label,
            m55_frequency_shifter_ports(),
            m55_frequency_shifter_params(),
            || Box::new(System55FrequencyShifter::default()),
        ),
        "m55-ring-modulator" => exact_system55_entry(
            m55_ring_component_id(),
            descriptor.label,
            m55_ring_ports(),
            m55_ring_params(),
            || Box::new(System55RingModulator::default()),
        ),
        "m55-928-sample-hold" => exact_system55_entry(
            m55_sample_hold_component_id(),
            descriptor.label,
            m55_sample_hold_ports(),
            m55_sample_hold_params(),
            || Box::new(System55SampleHold::default()),
        ),
        "m55-960-sequential-controller" => exact_system55_entry(
            m55_sequencer_component_id(),
            descriptor.label,
            m55_sequencer_ports(),
            m55_sequencer_params(),
            || Box::new(System55Sequencer::default()),
        ),
        "m55-961-interface" => exact_system55_entry(
            m55_interface_component_id(),
            descriptor.label,
            m55_interface_ports(),
            m55_interface_params(),
            || Box::new(System55Interface::default()),
        ),
        "m55-cp3a-mixer" => exact_system55_entry(
            m55_mixer_component_id(),
            descriptor.label,
            m55_mixer_ports(),
            m55_mixer_params(),
            || Box::new(System55Mixer::default()),
        ),
        "m55-multiple" => exact_system55_entry(
            m55_multiple_component_id(),
            descriptor.label,
            m55_multiple_ports(),
            m55_multiple_params(),
            || Box::new(System55Multiple::default()),
        ),
        "m55-attenuator" => exact_system55_entry(
            m55_attenuator_component_id(),
            descriptor.label,
            m55_attenuator_ports(),
            m55_attenuator_params(),
            || Box::new(System55Attenuator::default()),
        ),
        "m55-956-ribbon-controller" => exact_system55_entry(
            m55_ribbon_component_id(),
            descriptor.label,
            m55_ribbon_ports(),
            m55_ribbon_params(),
            || Box::new(System55Ribbon::default()),
        ),
        "m55-951-keyboard-controller" => exact_system55_entry(
            m55_keyboard_component_id(),
            descriptor.label,
            m55_keyboard_ports(),
            m55_keyboard_params(),
            || Box::new(System55Keyboard::default()),
        ),
        _ => compatible_system55_entry(descriptor),
    }
}

fn exact_system55_entry(
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
        InstrumentWrapperCategory::ModularAnalog,
    )
    .with_capability(ComponentCapability::RealtimeSafe)
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::Traceable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(ports)
    .with_params(params)
    .with_factory(factory)
}

fn compatible_system55_entry(descriptor: System55ModuleDescriptor) -> ComponentRegistryEntry {
    ComponentRegistryEntry::new(
        descriptor.id(),
        descriptor.label,
        ComponentRegistryCategory::Compatible,
        InstrumentWrapperCategory::ModularAnalog,
    )
    .with_capability(ComponentCapability::Editable)
    .with_capability(ComponentCapability::SpecializedView)
    .with_ports(descriptor.ports())
    .with_params(descriptor.params())
}
