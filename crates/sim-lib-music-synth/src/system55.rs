use sim_kernel::{Expr, Symbol};

pub use crate::modules::coupler::{
    System55FilterCoupler, System55FilterCouplerSettings, m55_coupler_component_id,
    m55_coupler_params, m55_coupler_ports,
};
pub use crate::modules::hpf::{
    System55HighPassFilter, System55HighPassFilterSettings, m55_hpf_component_id, m55_hpf_params,
    m55_hpf_ports,
};
pub use crate::modules::ladder::{
    System55LadderLpf, System55LadderLpfSettings, m55_ladder_lpf_component_id,
    m55_ladder_lpf_params, m55_ladder_lpf_ports,
};
pub use crate::modules::m55_amp_control::{
    System55Envelope, System55EnvelopeSettings, System55EnvelopeStage, System55TriggerDelay,
    System55TriggerDelayFrame, System55TriggerDelaySettings, System55Vca, System55VcaResponse,
    System55VcaSettings, m55_envelope_component_id, m55_envelope_params, m55_envelope_ports,
    m55_trigger_delay_component_id, m55_trigger_delay_params, m55_trigger_delay_ports,
    m55_vca_component_id, m55_vca_params, m55_vca_ports,
};
pub use crate::modules::m55_noise::{
    System55Noise, System55NoiseColor, System55NoiseFrame, System55NoiseSettings,
    m55_noise_component_id, m55_noise_params, m55_noise_ports,
};
pub use crate::modules::m55_play_control::{
    System55Interface, System55Keyboard, System55KeyboardFrame, System55KeyboardSettings,
    System55Ribbon, System55RibbonFrame, System55RibbonSettings, m55_interface_component_id,
    m55_interface_params, m55_interface_ports, m55_keyboard_component_id, m55_keyboard_params,
    m55_keyboard_ports, m55_ribbon_component_id, m55_ribbon_params, m55_ribbon_ports,
};
pub use crate::modules::m55_sample_control::{
    System55EnvelopeFollower, System55EnvelopeFollowerFrame, System55EnvelopeFollowerSettings,
    System55SampleHold, System55SampleHoldSettings, System55Sequencer, System55SequencerFrame,
    System55SequencerSettings, m55_env_follower_component_id, m55_env_follower_params,
    m55_env_follower_ports, m55_sample_hold_component_id, m55_sample_hold_params,
    m55_sample_hold_ports, m55_sequencer_component_id, m55_sequencer_params, m55_sequencer_ports,
};
pub use crate::modules::m55_spectral_utility::{
    SYSTEM55_FIXED_FILTER_BAND_COUNT, SYSTEM55_FIXED_FILTER_BANK_CENTERS_HZ,
    System55FixedFilterBank, System55FixedFilterBankFrame, System55FixedFilterBankSettings,
    System55FrequencyShifter, System55FrequencyShifterFrame, System55FrequencyShifterSettings,
    System55RingModulator, System55RingModulatorSettings, m55_fixed_filter_bank_component_id,
    m55_fixed_filter_bank_params, m55_fixed_filter_bank_ports, m55_frequency_shifter_component_id,
    m55_frequency_shifter_params, m55_frequency_shifter_ports, m55_ring_component_id,
    m55_ring_params, m55_ring_ports,
};
pub use crate::modules::m55_utility::{
    System55Attenuator, System55AttenuatorSettings, System55Mixer, System55MixerSettings,
    System55Multiple, System55MultipleFrame, System55MultipleSettings, m55_attenuator_component_id,
    m55_attenuator_params, m55_attenuator_ports, m55_mixer_component_id, m55_mixer_params,
    m55_mixer_ports, m55_multiple_component_id, m55_multiple_params, m55_multiple_ports,
};
pub use crate::modules::m55_vco::{
    System55Vco, System55VcoSettings, System55VcoWaveform, m55_vco_component_id, m55_vco_params,
    m55_vco_ports,
};
pub use crate::modules::vco_driver::{
    System55VcoDriver, System55VcoDriverFrame, System55VcoDriverSettings,
    m55_vco_driver_component_id, m55_vco_driver_params, m55_vco_driver_ports,
};
use crate::{
    ComponentParamDescriptor, ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection,
    ComponentPortMedia, GateConvention, GateConverter, GateFrame, GateMode, InstrumentPatch,
    PatchCord, PatchEndpoint, PatchJack, PatchModule,
};

/// Repository-relative path to the System 55 recipe book manifest.
pub const SYSTEM55_RECIPE_BOOK_PATH: &str = "crates/sim-lib-music-synth/recipes/system55/book.toml";
/// Repository-relative path to the System 55 scaffold recipe chapter.
pub const SYSTEM55_RECIPE_CHAPTER_PATH: &str =
    "crates/sim-lib-music-synth/recipes/system55/chapter.toml";
/// Fixture names covering the System 55 oscillator and noise sources.
pub const SYSTEM55_OSCILLATOR_FIXTURE_NAMES: [&str; 4] = [
    "system55-m55-vco-driver-fanout",
    "system55-m55-vco-pitch-sync-pwm",
    "system55-m55-vco-shared-driver-tracking",
    "system55-m55-noise-white-pink-bands",
];
/// Fixture names covering the System 55 ladder, high-pass, and coupler filters.
pub const SYSTEM55_FILTER_FIXTURE_NAMES: [&str; 6] = [
    "system55-m55-ladder-lpf-slope",
    "system55-m55-ladder-lpf-resonance-peak",
    "system55-m55-ladder-lpf-self-oscillation",
    "system55-m55-ladder-lpf-saturation",
    "system55-m55-hpf-slope",
    "system55-m55-coupler-bandpass",
];
/// Human-readable modeling notes describing each System 55 filter module.
pub const SYSTEM55_FILTER_MODEL_NOTES: [&str; 3] = [
    "m55-ladder-lpf: saturated four-pole cascade with feedback, cutoff CV, default 4x oversampling, and tuned self-oscillation above resonance 1.0.",
    "m55-hpf: cascaded high-pass poles with cutoff CV and bounded input drive.",
    "m55-coupler: high-pass into low-pass band window with independent cutoff CV inputs and bounded resonance.",
];
/// Fixture names covering the System 55 control, utility, and spectral modules.
pub const SYSTEM55_CONTROL_FIXTURE_NAMES: [&str; 12] = [
    "system55-m55-vca-gain-law",
    "system55-m55-envelope-s-trigger-timing",
    "system55-m55-trigger-delay-s-trigger-timing",
    "system55-m55-envelope-follower-gate",
    "system55-m55-fixed-filter-bank-centers",
    "system55-m55-frequency-shifter-sidebands",
    "system55-m55-ring-modulator-sidebands",
    "system55-m55-mixer-mix-behavior",
    "system55-m55-multiple-attenuator-utility",
    "system55-m55-sample-hold-s-trigger",
    "system55-m55-sequencer-advance",
    "system55-m55-ribbon-keyboard-mapping",
];

const SYSTEM55_MODULE_DESCRIPTORS: [System55ModuleDescriptor; 21] = [
    System55ModuleDescriptor::new(
        "m55-921a-oscillator-driver",
        "M55 921A Oscillator Driver",
        System55ModuleRole::Oscillator,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-921b-oscillator",
        "M55 921B Oscillator",
        System55ModuleRole::Oscillator,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-923-noise-filter",
        "M55 923 Noise and Filter",
        System55ModuleRole::Noise,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-904a-low-pass-filter",
        "M55 904A Low Pass Filter",
        System55ModuleRole::Filter,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-904b-high-pass-filter",
        "M55 904B High Pass Filter",
        System55ModuleRole::Filter,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-904c-filter-coupler",
        "M55 904C Filter Coupler",
        System55ModuleRole::Filter,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-907-fixed-filter-bank",
        "M55 907 Fixed Filter Bank",
        System55ModuleRole::Filter,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-902-vca",
        "M55 902 VCA",
        System55ModuleRole::Amplifier,
        Some(GateMode::STrigger),
    ),
    System55ModuleDescriptor::new(
        "m55-911-envelope-generator",
        "M55 911 Envelope Generator",
        System55ModuleRole::Envelope,
        Some(GateMode::STrigger),
    ),
    System55ModuleDescriptor::new(
        "m55-911a-dual-trigger-delay",
        "M55 911A Dual Trigger Delay",
        System55ModuleRole::Control,
        Some(GateMode::STrigger),
    ),
    System55ModuleDescriptor::new(
        "m55-912-envelope-follower",
        "M55 912 Envelope Follower",
        System55ModuleRole::Control,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-1630-frequency-shifter",
        "M55 1630 Frequency Shifter",
        System55ModuleRole::Utility,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-ring-modulator",
        "M55 Ring Modulator",
        System55ModuleRole::Utility,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-928-sample-hold",
        "M55 928 Sample and Hold",
        System55ModuleRole::Control,
        Some(GateMode::STrigger),
    ),
    System55ModuleDescriptor::new(
        "m55-960-sequential-controller",
        "M55 960 Sequential Controller",
        System55ModuleRole::Control,
        Some(GateMode::STrigger),
    ),
    System55ModuleDescriptor::new(
        "m55-961-interface",
        "M55 961 Interface",
        System55ModuleRole::Utility,
        Some(GateMode::STrigger),
    ),
    System55ModuleDescriptor::new(
        "m55-cp3a-mixer",
        "M55 CP3A Mixer",
        System55ModuleRole::Utility,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-multiple",
        "M55 Multiple",
        System55ModuleRole::Utility,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-attenuator",
        "M55 Attenuator",
        System55ModuleRole::Utility,
        None,
    ),
    System55ModuleDescriptor::new(
        "m55-956-ribbon-controller",
        "M55 956 Ribbon Controller",
        System55ModuleRole::Control,
        Some(GateMode::STrigger),
    ),
    System55ModuleDescriptor::new(
        "m55-951-keyboard-controller",
        "M55 951 Keyboard Controller",
        System55ModuleRole::Control,
        Some(GateMode::STrigger),
    ),
];

/// Functional role a System 55 module plays in the modular signal chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System55ModuleRole {
    /// Pitched tone source (oscillator driver or oscillator bank).
    Oscillator,
    /// Broadband noise source.
    Noise,
    /// Frequency-shaping filter (low-pass, high-pass, coupler, or bank).
    Filter,
    /// Voltage-controlled amplifier.
    Amplifier,
    /// Envelope generator.
    Envelope,
    /// Control-signal module (trigger delay, sample-and-hold, sequencer, controllers).
    Control,
    /// Signal-routing or processing utility (mixer, multiple, attenuator, shifter, interface).
    Utility,
}

impl System55ModuleRole {
    /// Returns the lowercase identifier string for this role.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oscillator => "oscillator",
            Self::Noise => "noise",
            Self::Filter => "filter",
            Self::Amplifier => "amplifier",
            Self::Envelope => "envelope",
            Self::Control => "control",
            Self::Utility => "utility",
        }
    }

    /// Returns the qualified `audio-synth/system55-role` symbol for this role.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/system55-role", self.as_str())
    }
}

/// Static description of one System 55 module: its identity, label, role, and
/// optional gate mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct System55ModuleDescriptor {
    /// Stable id-name fragment used to build the module's qualified symbol.
    pub id_name: &'static str,
    /// Human-readable module label (for example, "M55 902 VCA").
    pub label: &'static str,
    /// Functional role this module plays in the signal chain.
    pub role: System55ModuleRole,
    /// Gate convention the module triggers on, when it accepts a gate.
    pub gate: Option<GateMode>,
}

impl System55ModuleDescriptor {
    /// Builds a descriptor from its id-name, label, role, and optional gate mode.
    const fn new(
        id_name: &'static str,
        label: &'static str,
        role: System55ModuleRole,
        gate: Option<GateMode>,
    ) -> Self {
        Self {
            id_name,
            label,
            role,
            gate,
        }
    }

    /// Returns the qualified `audio-synth/module` symbol identifying this module.
    pub fn id(self) -> Symbol {
        m55_module_id(self.id_name)
    }

    /// Returns the port descriptors implied by this module's role.
    pub fn ports(self) -> Vec<ComponentPortDescriptor> {
        system55_module_ports(self.role)
    }

    /// Returns the parameter descriptors for this module: a role parameter plus
    /// a gate-mode parameter when the module accepts a gate.
    pub fn params(self) -> Vec<ComponentParamDescriptor> {
        let mut params = vec![
            ComponentParamDescriptor::new(
                Symbol::qualified("audio-synth/param", format!("{}-role", self.id_name)),
                "Module role",
                ComponentParamUnit::Unitless,
            )
            .with_enum_values(vec![self.role.symbol()], 0),
        ];
        if self.gate.is_some() {
            params.push(
                ComponentParamDescriptor::new(
                    Symbol::qualified("audio-synth/param", format!("{}-gate-mode", self.id_name)),
                    "Gate mode",
                    ComponentParamUnit::Unitless,
                )
                .with_enum_values(system55_gate_mode_symbols().to_vec(), 0),
            );
        }
        params
    }
}

/// Evidence that the System 55 S-trigger modules fit the shared gate convention,
/// recording the native and voltage-gate levels for the inactive and active states.
#[derive(Clone, Debug, PartialEq)]
pub struct System55STriggerFitEvidence {
    /// Gate mode the System 55 modules use (S-trigger).
    pub gate_mode: GateMode,
    /// Native trigger voltage in the inactive state.
    pub native_inactive_voltage_v: f32,
    /// Native trigger voltage in the active state.
    pub native_active_voltage_v: f32,
    /// Voltage-gate level corresponding to the native inactive voltage.
    pub voltage_gate_inactive_v: f32,
    /// Voltage-gate level corresponding to the native active voltage.
    pub voltage_gate_active_v: f32,
    /// Repository path to the scaffold recipe chapter documenting the fit.
    pub scaffold_chapter_path: &'static str,
}

/// Returns the descriptors for all 21 modeled System 55 modules.
pub fn system55_module_descriptors() -> Vec<System55ModuleDescriptor> {
    SYSTEM55_MODULE_DESCRIPTORS.to_vec()
}

/// Returns the qualified module symbols for all modeled System 55 modules.
pub fn system55_module_ids() -> Vec<Symbol> {
    system55_module_descriptors()
        .into_iter()
        .map(System55ModuleDescriptor::id)
        .collect()
}

/// Returns the module ids for the System 55 oscillator sources (driver, VCO, noise).
pub fn system55_oscillator_module_ids() -> [Symbol; 3] {
    [
        m55_vco_driver_component_id(),
        m55_vco_component_id(),
        m55_noise_component_id(),
    ]
}

/// Returns the fixture names exercising the System 55 oscillator sources.
pub fn system55_oscillator_fixture_names() -> [&'static str; 4] {
    SYSTEM55_OSCILLATOR_FIXTURE_NAMES
}

/// Returns the module ids for the System 55 filters (ladder LPF, HPF, coupler).
pub fn system55_filter_module_ids() -> [Symbol; 3] {
    [
        m55_ladder_lpf_component_id(),
        m55_hpf_component_id(),
        m55_coupler_component_id(),
    ]
}

/// Returns the fixture names exercising the System 55 filters.
pub fn system55_filter_fixture_names() -> [&'static str; 6] {
    SYSTEM55_FILTER_FIXTURE_NAMES
}

/// Returns the modeling notes describing each System 55 filter.
pub fn system55_filter_model_notes() -> [&'static str; 3] {
    SYSTEM55_FILTER_MODEL_NOTES
}

/// Returns the module ids for the System 55 control, utility, and spectral modules.
pub fn system55_control_module_ids() -> [Symbol; 15] {
    [
        m55_vca_component_id(),
        m55_envelope_component_id(),
        m55_trigger_delay_component_id(),
        m55_env_follower_component_id(),
        m55_fixed_filter_bank_component_id(),
        m55_frequency_shifter_component_id(),
        m55_ring_component_id(),
        m55_mixer_component_id(),
        m55_multiple_component_id(),
        m55_attenuator_component_id(),
        m55_sample_hold_component_id(),
        m55_sequencer_component_id(),
        m55_interface_component_id(),
        m55_ribbon_component_id(),
        m55_keyboard_component_id(),
    ]
}

/// Returns the fixture names exercising the System 55 control and utility modules.
pub fn system55_control_fixture_names() -> [&'static str; 12] {
    SYSTEM55_CONTROL_FIXTURE_NAMES
}

/// Returns the qualified patch id for the System 55 scaffold patch.
pub fn system55_scaffold_patch_id() -> Symbol {
    Symbol::qualified("audio-synth/patch", "moog-system-55-scaffold")
}

/// Returns the gate-mode symbols supported by System 55 modules (S-trigger and
/// voltage gate).
pub fn system55_gate_mode_symbols() -> [Symbol; 2] {
    [
        GateConvention::s_trigger().mode().symbol(),
        GateConvention::voltage_gate().mode().symbol(),
    ]
}

/// Returns the S-trigger gate convention used across System 55 modules.
pub fn system55_s_trigger_convention() -> GateConvention {
    GateConvention::s_trigger()
}

/// Returns the recorded fit evidence for the System 55 S-trigger convention.
pub fn system55_s_trigger_fit_evidence() -> System55STriggerFitEvidence {
    let convention = system55_s_trigger_convention();
    System55STriggerFitEvidence {
        gate_mode: convention.mode(),
        native_inactive_voltage_v: convention.native_voltage(false),
        native_active_voltage_v: convention.native_voltage(true),
        voltage_gate_inactive_v: convention.voltage_gate_voltage(convention.native_voltage(false)),
        voltage_gate_active_v: convention.voltage_gate_voltage(convention.native_voltage(true)),
        scaffold_chapter_path: SYSTEM55_RECIPE_CHAPTER_PATH,
    }
}

/// Converts a series of input voltages through the System 55 S-trigger convention,
/// returning one gate frame per input voltage.
pub fn system55_s_trigger_voltage_gate_frames(input_volts: &[f32]) -> Vec<GateFrame> {
    let mut converter = GateConverter::new(system55_s_trigger_convention());
    input_volts
        .iter()
        .map(|volts| converter.convert(*volts))
        .collect()
}

/// Builds the System 55 scaffold patch: a minimal oscillator-to-VCA voice with
/// an envelope generator and S-trigger interface, used as a wiring reference.
pub fn system55_scaffold_patch() -> InstrumentPatch {
    InstrumentPatch::new(system55_scaffold_patch_id())
        .with_module(scaffold_module(
            "osc-driver-1",
            "m55-921a-oscillator-driver",
            System55ModuleRole::Oscillator,
            vec![PatchJack::cv("keyboard-cv-in", false)],
            vec![PatchJack::cv("pitch-cv-out", true)],
            None,
        ))
        .with_module(scaffold_module(
            "osc-bank-1",
            "m55-921b-oscillator",
            System55ModuleRole::Oscillator,
            vec![PatchJack::cv("pitch-cv-in", true)],
            vec![PatchJack::audio("audio-out", true)],
            None,
        ))
        .with_module(scaffold_module(
            "filter-1",
            "m55-904a-low-pass-filter",
            System55ModuleRole::Filter,
            vec![
                PatchJack::audio("audio-in", true),
                PatchJack::cv("cutoff-cv-in", false),
            ],
            vec![PatchJack::audio("audio-out", true)],
            None,
        ))
        .with_module(scaffold_module(
            "vca-1",
            "m55-902-vca",
            System55ModuleRole::Amplifier,
            vec![
                PatchJack::audio("audio-in", true),
                PatchJack::cv("gain-cv-in", false),
            ],
            vec![PatchJack::audio("audio-out", true)],
            Some(system55_s_trigger_convention()),
        ))
        .with_module(scaffold_module(
            "eg-1",
            "m55-911-envelope-generator",
            System55ModuleRole::Envelope,
            vec![PatchJack::gate("s-trigger-in", true)],
            vec![PatchJack::cv("envelope-cv-out", true)],
            Some(system55_s_trigger_convention()),
        ))
        .with_module(scaffold_module(
            "interface-1",
            "m55-961-interface",
            System55ModuleRole::Utility,
            vec![PatchJack::gate("s-trigger-in", false)],
            vec![PatchJack::gate("voltage-gate-out", false)],
            Some(system55_s_trigger_convention()),
        ))
        .with_cord(cord(
            "osc-driver-1",
            "pitch-cv-out",
            "osc-bank-1",
            "pitch-cv-in",
        ))
        .with_cord(cord("osc-bank-1", "audio-out", "filter-1", "audio-in"))
        .with_cord(cord("filter-1", "audio-out", "vca-1", "audio-in"))
        .with_cord(cord("eg-1", "envelope-cv-out", "vca-1", "gain-cv-in"))
        .with_setting(
            Symbol::qualified("audio-synth/system55", "module-ids"),
            Expr::Vector(
                SYSTEM55_MODULE_DESCRIPTORS
                    .iter()
                    .map(|descriptor| Expr::Symbol(descriptor.id()))
                    .collect(),
            ),
        )
        .with_setting(
            Symbol::qualified("audio-synth/system55", "gate-mode"),
            Expr::Symbol(system55_s_trigger_convention().mode().symbol()),
        )
}

fn m55_module_id(id_name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/module", id_name)
}

fn module_instance_id(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/system55", name)
}

fn system55_module_ports(role: System55ModuleRole) -> Vec<ComponentPortDescriptor> {
    match role {
        System55ModuleRole::Oscillator => vec![
            port(
                "pitch-cv-in",
                ComponentPortMedia::ControlVoltage,
                ComponentPortDirection::Input,
            )
            .optional(),
            port(
                "audio-out",
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Output,
            ),
        ],
        System55ModuleRole::Noise => {
            vec![port(
                "audio-out",
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Output,
            )]
        }
        System55ModuleRole::Filter => vec![
            port(
                "audio-in",
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Input,
            ),
            port(
                "cutoff-cv-in",
                ComponentPortMedia::ControlVoltage,
                ComponentPortDirection::Input,
            )
            .optional(),
            port(
                "audio-out",
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Output,
            ),
        ],
        System55ModuleRole::Amplifier => vec![
            port(
                "audio-in",
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Input,
            ),
            port(
                "gain-cv-in",
                ComponentPortMedia::ControlVoltage,
                ComponentPortDirection::Input,
            )
            .optional(),
            port(
                "audio-out",
                ComponentPortMedia::AudioRate,
                ComponentPortDirection::Output,
            ),
        ],
        System55ModuleRole::Envelope => vec![
            port(
                "s-trigger-in",
                ComponentPortMedia::Gate,
                ComponentPortDirection::Input,
            ),
            port(
                "envelope-cv-out",
                ComponentPortMedia::ControlVoltage,
                ComponentPortDirection::Output,
            ),
        ],
        System55ModuleRole::Control => vec![
            port(
                "s-trigger-in",
                ComponentPortMedia::Gate,
                ComponentPortDirection::Input,
            ),
            port(
                "gate-out",
                ComponentPortMedia::Gate,
                ComponentPortDirection::Output,
            ),
        ],
        System55ModuleRole::Utility => vec![
            port(
                "signal-in",
                ComponentPortMedia::ControlVoltage,
                ComponentPortDirection::Input,
            )
            .optional(),
            port(
                "signal-out",
                ComponentPortMedia::ControlVoltage,
                ComponentPortDirection::Output,
            )
            .optional(),
        ],
    }
}

fn port(
    name: &'static str,
    media: ComponentPortMedia,
    direction: ComponentPortDirection,
) -> ComponentPortDescriptor {
    ComponentPortDescriptor::new(
        Symbol::qualified("audio-synth/port", name),
        media,
        direction,
        1,
    )
}

fn scaffold_module(
    instance_name: &'static str,
    module_id: &'static str,
    role: System55ModuleRole,
    inputs: Vec<PatchJack>,
    outputs: Vec<PatchJack>,
    gate: Option<GateConvention>,
) -> PatchModule {
    let mut module = PatchModule::new(module_instance_id(instance_name), m55_module_id(module_id))
        .with_setting(
            Symbol::qualified("audio-synth/system55", "role"),
            Expr::Symbol(role.symbol()),
        );
    if let Some(gate) = gate {
        module = module.with_setting(
            Symbol::qualified("audio-synth/system55", "gate-mode"),
            Expr::Symbol(gate.mode().symbol()),
        );
    }
    for input in inputs {
        module = module.with_input(input);
    }
    for output in outputs {
        module = module.with_output(output);
    }
    module
}

fn cord(
    from_module: &'static str,
    from_jack: &'static str,
    to_module: &'static str,
    to_jack: &'static str,
) -> PatchCord {
    PatchCord::new(
        endpoint(from_module, from_jack),
        endpoint(to_module, to_jack),
    )
}

fn endpoint(module: &'static str, jack: &'static str) -> PatchEndpoint {
    PatchEndpoint {
        module: module_instance_id(module),
        jack: Symbol::new(jack),
    }
}
