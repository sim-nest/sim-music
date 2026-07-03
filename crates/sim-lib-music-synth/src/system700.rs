use sim_kernel::{Expr, Symbol};

use crate::{
    ComponentParamDescriptor, ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection,
    ComponentPortMedia, InstrumentPatch, PatchCord, PatchEndpoint, PatchJack, PatchModule,
};

pub use crate::system700_wrapper::{System700, system700_audio_graph};

pub use crate::modules::clock::{
    System700Clock, System700ClockFrame, System700ClockSettings, r700_clock_component_id,
    r700_clock_params, r700_clock_ports,
};
pub use crate::modules::envelope::{
    System700Envelope, System700EnvelopeSettings, System700EnvelopeStage,
    r700_envelope_component_id, r700_envelope_params, r700_envelope_ports,
};
pub use crate::modules::extin::{
    System700ExternalInput, System700ExternalInputFrame, System700ExternalInputSettings,
    r700_external_input_component_id, r700_external_input_params, r700_external_input_ports,
};
pub use crate::modules::keyboard::{
    System700Keyboard, System700KeyboardFrame, System700KeyboardSettings,
    r700_keyboard_component_id, r700_keyboard_params, r700_keyboard_ports,
};
pub use crate::modules::lfo::{
    System700Lfo, System700LfoSettings, System700LfoWaveform, r700_lfo_component_id,
    r700_lfo_params, r700_lfo_ports,
};
pub use crate::modules::mixer::{
    System700Mixer, System700MixerSettings, r700_mixer_component_id, r700_mixer_params,
    r700_mixer_ports,
};
pub use crate::modules::mult::{
    System700Multiple, System700MultipleSettings, r700_multiple_component_id, r700_multiple_params,
    r700_multiple_ports,
};
pub use crate::modules::noise::{
    System700Noise, System700NoiseColor, System700NoiseSettings, r700_noise_component_id,
    r700_noise_params, r700_noise_ports,
};
pub use crate::modules::ring::{
    System700RingModulator, System700RingSettings, r700_ring_component_id, r700_ring_params,
    r700_ring_ports,
};
pub use crate::modules::sample_hold::{
    System700SampleHold, System700SampleHoldSettings, r700_sample_hold_component_id,
    r700_sample_hold_params, r700_sample_hold_ports,
};
pub use crate::modules::sequencer::{
    System700Sequencer, System700SequencerFrame, System700SequencerSettings,
    r700_sequencer_component_id, r700_sequencer_params, r700_sequencer_ports,
};
pub use crate::modules::vca::{
    System700Vca, System700VcaResponse, System700VcaSettings, r700_vca_component_id,
    r700_vca_params, r700_vca_ports,
};
pub use crate::modules::vcf::{
    System700Vcf, System700VcfMode, System700VcfSettings, r700_vcf_component_id,
    r700_vcf_mode_symbols, r700_vcf_params, r700_vcf_ports,
};
pub use crate::modules::vco::{
    System700Vco, System700VcoSettings, System700VcoWaveform, r700_vco_component_id,
    r700_vco_params, r700_vco_ports,
};
pub use crate::modules::vproc::{
    System700VoltageProcessor, System700VoltageProcessorSettings,
    r700_voltage_processor_component_id, r700_voltage_processor_params,
    r700_voltage_processor_ports,
};

/// Names of the System 700 source-module fixtures (VCO, LFO, and noise behaviors).
pub const SYSTEM700_SOURCE_FIXTURE_NAMES: [&str; 4] = [
    "system700-r700-vco-pitch-sync-pwm",
    "system700-r700-vco-exponential-fm",
    "system700-r700-lfo-delay-rate-cv",
    "system700-r700-noise-color-bands",
];

/// Names of the System 700 shaper-module fixtures (VCF, VCA, and ring behaviors).
pub const SYSTEM700_SHAPER_FIXTURE_NAMES: [&str; 5] = [
    "system700-r700-vcf-cutoff-tracking",
    "system700-r700-vcf-resonance-self-oscillation",
    "system700-r700-vca-gain-law",
    "system700-r700-vca-saturation",
    "system700-r700-ring-sidebands",
];

/// Names of the System 700 control-module fixtures (envelope, sample-hold,
/// voltage processor, mixer, external input, keyboard, clock, and sequencer).
pub const SYSTEM700_CONTROL_FIXTURE_NAMES: [&str; 8] = [
    "system700-r700-envelope-timing",
    "system700-r700-sample-hold-capture",
    "system700-r700-voltage-processor-transfer",
    "system700-r700-mixer-multiple",
    "system700-r700-external-input-map",
    "system700-r700-keyboard-cv-gate",
    "system700-r700-clock-pulses",
    "system700-r700-sequencer-steps",
];

/// Filesystem path where the System 700 user patch for the main console is stored.
pub const SYSTEM700_USER_PATCH_PATH: &str =
    "$HOME/.local/share/sim/system700/main-console.patch.siml";

/// Render fidelity selected for a System 700 instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System700RenderMode {
    /// Clean, algorithmic output without analog modeling.
    Ideal,
    /// Modeled output with analog-style nonlinearity.
    Modeled,
    /// Modeled output that also emits per-frame trace data.
    Trace,
}

impl System700RenderMode {
    /// Returns the stable string name of this render mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ideal => "ideal",
            Self::Modeled => "modeled",
            Self::Trace => "trace",
        }
    }

    /// Returns the qualified [`Symbol`] naming this render mode.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/system700-render-mode", self.as_str())
    }
}

/// Returns the component ids of the System 700 source modules (VCO, LFO, noise).
pub fn system700_source_module_ids() -> [Symbol; 3] {
    [
        r700_vco_component_id(),
        r700_lfo_component_id(),
        r700_noise_component_id(),
    ]
}

/// Returns the names of the System 700 source-module fixtures.
pub fn system700_source_fixture_names() -> [&'static str; 4] {
    SYSTEM700_SOURCE_FIXTURE_NAMES
}

/// Returns the component ids of the System 700 shaper modules (VCF, VCA, ring).
pub fn system700_shaper_module_ids() -> [Symbol; 3] {
    [
        r700_vcf_component_id(),
        r700_vca_component_id(),
        r700_ring_component_id(),
    ]
}

/// Returns the names of the System 700 shaper-module fixtures.
pub fn system700_shaper_fixture_names() -> [&'static str; 5] {
    SYSTEM700_SHAPER_FIXTURE_NAMES
}

/// Returns the qualified symbols for every System 700 VCF mode.
pub fn system700_vcf_mode_symbols() -> [Symbol; 4] {
    r700_vcf_mode_symbols()
}

/// Returns the component ids of the System 700 control modules (envelope,
/// sample-hold, voltage processor, mixer, multiple, external input, keyboard,
/// clock, sequencer).
pub fn system700_control_module_ids() -> [Symbol; 9] {
    [
        r700_envelope_component_id(),
        r700_sample_hold_component_id(),
        r700_voltage_processor_component_id(),
        r700_mixer_component_id(),
        r700_multiple_component_id(),
        r700_external_input_component_id(),
        r700_keyboard_component_id(),
        r700_clock_component_id(),
        r700_sequencer_component_id(),
    ]
}

/// Returns the names of the System 700 control-module fixtures.
pub fn system700_control_fixture_names() -> [&'static str; 8] {
    SYSTEM700_CONTROL_FIXTURE_NAMES
}

/// Returns the component id symbol for the System 700 instrument.
pub fn system700_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "roland-system-700")
}

/// Returns the patch id symbol for the System 700 default main-console patch.
pub fn system700_default_patch_id() -> Symbol {
    Symbol::qualified("audio-synth/patch", "roland-system-700-main-console")
}

/// Returns the filesystem path of the System 700 user patch.
pub fn system700_user_patch_path() -> &'static str {
    SYSTEM700_USER_PATCH_PATH
}

/// Returns the qualified symbols for every [`System700RenderMode`].
pub fn system700_render_mode_symbols() -> [Symbol; 3] {
    [
        System700RenderMode::Ideal.symbol(),
        System700RenderMode::Modeled.symbol(),
        System700RenderMode::Trace.symbol(),
    ]
}

/// Returns the component ids of every module the System 700 requires, across
/// the source, shaper, and control groups.
pub fn system700_required_module_ids() -> Vec<Symbol> {
    let mut ids = Vec::new();
    ids.extend(system700_source_module_ids());
    ids.extend(system700_shaper_module_ids());
    ids.extend(system700_control_module_ids());
    ids
}

/// Returns the port descriptors for the System 700 instrument.
pub fn system700_ports() -> Vec<ComponentPortDescriptor> {
    vec![
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "midi-in"),
            ComponentPortMedia::Event,
            ComponentPortDirection::Input,
            1,
        )
        .optional(),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "audio-out"),
            ComponentPortMedia::AudioRate,
            ComponentPortDirection::Output,
            1,
        ),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "patch-out"),
            ComponentPortMedia::Metadata,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
        ComponentPortDescriptor::new(
            Symbol::qualified("audio-synth/port", "trace-out"),
            ComponentPortMedia::Trace,
            ComponentPortDirection::Output,
            1,
        )
        .optional(),
    ]
}

/// Returns the parameter descriptors for the System 700 instrument.
pub fn system700_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "system700-render-mode"),
            "Render mode",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(system700_render_mode_symbols().to_vec(), 1),
    ]
}

/// Returns the default System 700 main-console patch (the default-voice profile).
pub fn system700_default_patch() -> InstrumentPatch {
    system700_main_console_patch(System700PatchProfile::DefaultVoice)
}

/// Returns a minimal single-module patch: one VCO with a pitch CV input.
pub fn system700_single_module_patch() -> InstrumentPatch {
    InstrumentPatch::new(Symbol::qualified(
        "audio-synth/patch",
        "roland-system-700-single-module",
    ))
    .with_module(patch_module(
        "vco-1",
        r700_vco_component_id(),
        "source",
        vec![PatchJack::cv("pitch-cv-in", false)],
        vec![PatchJack::audio("audio-out", true)],
    ))
    .with_setting(profile_key(), Expr::String("single-module".to_owned()))
}

/// Returns a two-module patch: a VCO feeding a VCF.
pub fn system700_two_module_patch() -> InstrumentPatch {
    InstrumentPatch::new(Symbol::qualified(
        "audio-synth/patch",
        "roland-system-700-two-module",
    ))
    .with_module(patch_module(
        "vco-1",
        r700_vco_component_id(),
        "source",
        vec![PatchJack::cv("pitch-cv-in", false)],
        vec![PatchJack::audio("audio-out", true)],
    ))
    .with_module(patch_module(
        "vcf-1",
        r700_vcf_component_id(),
        "filter",
        vec![
            PatchJack::audio("audio-in", true),
            PatchJack::cv("cutoff-cv-in", false),
        ],
        vec![PatchJack::audio("audio-out", true)],
    ))
    .with_cord(PatchCord::new(
        PatchEndpoint::new("vco-1", "audio-out"),
        PatchEndpoint::new("vcf-1", "audio-in"),
    ))
    .with_setting(profile_key(), Expr::String("two-module".to_owned()))
}

/// Returns the main-console patch configured for the sequencer-driven profile.
pub fn system700_sequencer_patch() -> InstrumentPatch {
    system700_main_console_patch(System700PatchProfile::SequencerDriven)
}

/// Returns the main-console patch configured for the patch round-trip profile.
pub fn system700_patch_round_trip_patch() -> InstrumentPatch {
    system700_main_console_patch(System700PatchProfile::PatchRoundTrip)
}

fn system700_main_console_patch(profile: System700PatchProfile) -> InstrumentPatch {
    InstrumentPatch::new(system700_default_patch_id())
        .with_module(patch_module(
            "keyboard-1",
            r700_keyboard_component_id(),
            "interface",
            vec![
                PatchJack::event("key-in", false),
                PatchJack::gate("gate-in", false),
                PatchJack::cv("bend-in", false),
            ],
            vec![
                PatchJack::cv("pitch-cv-out", true),
                PatchJack::gate("gate-out", true),
                PatchJack::gate("trigger-out", false),
            ],
        ))
        .with_module(patch_module(
            "vco-1",
            r700_vco_component_id(),
            "source",
            vec![PatchJack::cv("pitch-cv-in", true)],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "vcf-1",
            r700_vcf_component_id(),
            "filter",
            vec![
                PatchJack::audio("audio-in", true),
                PatchJack::cv("cutoff-cv-in", false),
            ],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "envelope-1",
            r700_envelope_component_id(),
            "contour",
            vec![PatchJack::gate("gate-in", true)],
            vec![PatchJack::cv("cv-out", true)],
        ))
        .with_module(patch_module(
            "vca-1",
            r700_vca_component_id(),
            "amplifier",
            vec![
                PatchJack::audio("audio-in", true),
                PatchJack::cv("gain-cv-in", true),
            ],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "mixer-1",
            r700_mixer_component_id(),
            "main-mixer",
            vec![PatchJack::audio("audio-in-1", true)],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "clock-1",
            r700_clock_component_id(),
            "clock",
            Vec::new(),
            vec![PatchJack::gate("trigger-out", true)],
        ))
        .with_module(patch_module(
            "sequencer-1",
            r700_sequencer_component_id(),
            "sequencer",
            vec![PatchJack::gate("clock-in", true)],
            vec![
                PatchJack::cv("cv-out", true),
                PatchJack::gate("gate-out", true),
            ],
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("keyboard-1", "pitch-cv-out"),
            PatchEndpoint::new("vco-1", "pitch-cv-in"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("keyboard-1", "gate-out"),
            PatchEndpoint::new("envelope-1", "gate-in"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("vco-1", "audio-out"),
            PatchEndpoint::new("vcf-1", "audio-in"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("envelope-1", "cv-out"),
            PatchEndpoint::new("vcf-1", "cutoff-cv-in"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("vcf-1", "audio-out"),
            PatchEndpoint::new("vca-1", "audio-in"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("envelope-1", "cv-out"),
            PatchEndpoint::new("vca-1", "gain-cv-in"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("vca-1", "audio-out"),
            PatchEndpoint::new("mixer-1", "audio-in-1"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("clock-1", "trigger-out"),
            PatchEndpoint::new("sequencer-1", "clock-in"),
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("sequencer-1", "cv-out"),
            PatchEndpoint::new("vco-1", "pitch-cv-in"),
        ))
        .with_setting(profile_key(), Expr::String(profile.as_str().to_owned()))
        .with_setting(
            Symbol::qualified("audio-synth/system700", "user-patch-path"),
            Expr::String(SYSTEM700_USER_PATCH_PATH.to_owned()),
        )
}

/// Returns a source-scaffold patch wiring a VCO, LFO, and noise source, tagged
/// with the source fixture names.
pub fn system700_scaffold_patch() -> InstrumentPatch {
    InstrumentPatch::new(Symbol::qualified(
        "audio-synth/patch",
        "system700-source-scaffold",
    ))
    .with_module(source_module(
        "vco-1",
        r700_vco_component_id(),
        "saw",
        System700VcoWaveform::Saw.symbol(),
        vec![
            PatchJack::cv("pitch-cv-in", false),
            PatchJack::cv("exp-fm-in", false),
            PatchJack::cv("pwm-cv-in", false),
            PatchJack::event("sync-in", false),
        ],
        vec![PatchJack::audio("audio-out", true)],
    ))
    .with_module(source_module(
        "lfo-1",
        r700_lfo_component_id(),
        "triangle",
        System700LfoWaveform::Triangle.symbol(),
        vec![PatchJack::cv("rate-cv-in", false)],
        vec![PatchJack::cv("cv-out", true)],
    ))
    .with_module(source_module(
        "noise-1",
        r700_noise_component_id(),
        "white",
        System700NoiseColor::White.symbol(),
        Vec::new(),
        vec![PatchJack::new(
            "audio-out",
            ComponentPortMedia::AudioRate,
            true,
        )],
    ))
    .with_setting(
        Symbol::qualified("audio-synth/system700", "source-fixtures"),
        Expr::Vector(
            SYSTEM700_SOURCE_FIXTURE_NAMES
                .iter()
                .map(|name| Expr::String((*name).to_owned()))
                .collect(),
        ),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum System700PatchProfile {
    SingleModule,
    TwoModule,
    DefaultVoice,
    SequencerDriven,
    PatchRoundTrip,
}

impl System700PatchProfile {
    pub(crate) fn from_patch(patch: &InstrumentPatch) -> Self {
        patch
            .settings
            .iter()
            .find_map(|setting| {
                (setting.key == profile_key()).then(|| match &setting.value {
                    Expr::String(value) => Some(Self::from_str(value)),
                    _ => None,
                })?
            })
            .unwrap_or(Self::DefaultVoice)
    }

    fn from_str(value: &str) -> Self {
        match value {
            "single-module" => Self::SingleModule,
            "two-module" => Self::TwoModule,
            "sequencer-driven" => Self::SequencerDriven,
            "patch-round-trip" => Self::PatchRoundTrip,
            _ => Self::DefaultVoice,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SingleModule => "single-module",
            Self::TwoModule => "two-module",
            Self::DefaultVoice => "default-voice",
            Self::SequencerDriven => "sequencer-driven",
            Self::PatchRoundTrip => "patch-round-trip",
        }
    }
}

fn patch_module(
    id: &'static str,
    kind: Symbol,
    role: &'static str,
    inputs: Vec<PatchJack>,
    outputs: Vec<PatchJack>,
) -> PatchModule {
    let mut module = PatchModule::new(Symbol::new(id), kind).with_setting(
        Symbol::qualified("audio-synth/system700", "role"),
        Expr::String(role.to_owned()),
    );
    for input in inputs {
        module = module.with_input(input);
    }
    for output in outputs {
        module = module.with_output(output);
    }
    module
}

fn source_module(
    id: &'static str,
    kind: Symbol,
    role: &'static str,
    shape: Symbol,
    inputs: Vec<PatchJack>,
    outputs: Vec<PatchJack>,
) -> PatchModule {
    let mut module = PatchModule::new(Symbol::qualified("audio-synth/r700", id), kind)
        .with_setting(
            Symbol::qualified("audio-synth/system700", "role"),
            Expr::String(role.to_owned()),
        )
        .with_setting(
            Symbol::qualified("audio-synth/system700", "shape"),
            Expr::Symbol(shape),
        );
    for input in inputs {
        module = module.with_input(input);
    }
    for output in outputs {
        module = module.with_output(output);
    }
    module
}

fn profile_key() -> Symbol {
    Symbol::qualified("audio-synth/system700", "profile")
}
