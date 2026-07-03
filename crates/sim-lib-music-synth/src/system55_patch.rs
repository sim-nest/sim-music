use sim_kernel::{Expr, Symbol};

use crate::system55::{
    m55_envelope_component_id, m55_fixed_filter_bank_component_id, m55_keyboard_component_id,
    m55_ladder_lpf_component_id, m55_mixer_component_id, m55_sequencer_component_id,
    m55_vca_component_id, m55_vco_component_id, m55_vco_driver_component_id,
};
use crate::{
    ComponentParamDescriptor, ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection,
    ComponentPortMedia, InstrumentPatch, PatchCord, PatchEndpoint, PatchJack, PatchModule,
};

/// Default filesystem path where a user's edited System 55 voice patch is stored.
pub const SYSTEM55_USER_PATCH_PATH: &str =
    "$HOME/.local/share/sim/system55/synthetic-voice.patch.siml";
/// Repository-relative path to the synthetic-voice recipe for the System 55.
pub const SYSTEM55_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/system55/synthetic-voice/recipe.toml";
/// The named tap points exposed on the default System 55 voice patch.
pub const SYSTEM55_PATCH_POINTS: [System55PatchPoint; 10] = [
    System55PatchPoint::new("keyboard-pitch-cv", "keyboard-1", "pitch-cv-out"),
    System55PatchPoint::new("keyboard-s-trigger", "keyboard-1", "s-trigger-out"),
    System55PatchPoint::new("driver-pitch-cv", "osc-driver-1", "pitch-cv-out"),
    System55PatchPoint::new("oscillator-stack-audio", "mixer-1", "audio-out"),
    System55PatchPoint::new("ladder-audio", "ladder-1", "audio-out"),
    System55PatchPoint::new("envelope-cv", "envelope-1", "envelope-cv-out"),
    System55PatchPoint::new("vca-audio", "vca-1", "audio-out"),
    System55PatchPoint::new("filter-bank-audio", "filter-bank-1", "audio-out"),
    System55PatchPoint::new("sequencer-cv", "sequencer-1", "cv-out"),
    System55PatchPoint::new("sequencer-s-trigger", "sequencer-1", "s-trigger-out"),
];

/// Fidelity level at which the System 55 voice is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System55RenderMode {
    /// Clean algorithmic reference render.
    Ideal,
    /// Analog-modeled render with saturation and nonlinearities.
    Modeled,
    /// Modeled render that also emits per-frame component traces.
    Trace,
}

impl System55RenderMode {
    /// Returns the lowercase identifier string for this render mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ideal => "ideal",
            Self::Modeled => "modeled",
            Self::Trace => "trace",
        }
    }

    /// Returns the qualified `audio-synth/system55-render-mode` symbol for this mode.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/system55-render-mode", self.as_str())
    }
}

/// A named tap point on the System 55 voice patch, identifying a module jack to
/// probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct System55PatchPoint {
    /// Stable name of the patch point.
    pub name: &'static str,
    /// Instance name of the module the jack belongs to.
    pub module: &'static str,
    /// Jack name on that module.
    pub jack: &'static str,
}

impl System55PatchPoint {
    /// Builds a patch point from its name and the module/jack it taps.
    pub const fn new(name: &'static str, module: &'static str, jack: &'static str) -> Self {
        Self { name, module, jack }
    }

    /// Returns the [`PatchEndpoint`] addressing this patch point's module jack.
    pub fn endpoint(self) -> PatchEndpoint {
        PatchEndpoint::new(self.module, self.jack)
    }
}

/// Selects which voice configuration the System 55 default patch builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System55PatchProfile {
    /// Bare stack of oscillators through the mixer.
    OscillatorStack,
    /// Ladder filter driven into self-oscillation.
    LadderSelfOscillation,
    /// Oscillator stack shaped by the fixed filter bank.
    FilterBank,
    /// Complete playable voice (oscillators, filter, envelope, VCA).
    DefaultVoice,
    /// Default voice driven by the internal step sequencer.
    SequencerDriven,
    /// Voice configuration used for patch serialization round-trips.
    PatchRoundTrip,
}

impl System55PatchProfile {
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
            "oscillator-stack" => Self::OscillatorStack,
            "ladder-self-oscillation" => Self::LadderSelfOscillation,
            "filter-bank" => Self::FilterBank,
            "sequencer-driven" => Self::SequencerDriven,
            "patch-round-trip" => Self::PatchRoundTrip,
            _ => Self::DefaultVoice,
        }
    }

    /// Returns the lowercase identifier string for this patch profile.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OscillatorStack => "oscillator-stack",
            Self::LadderSelfOscillation => "ladder-self-oscillation",
            Self::FilterBank => "filter-bank",
            Self::DefaultVoice => "default-voice",
            Self::SequencerDriven => "sequencer-driven",
            Self::PatchRoundTrip => "patch-round-trip",
        }
    }
}

/// Returns the qualified instrument id for the Moog System 55 voice.
pub fn system55_component_id() -> Symbol {
    Symbol::qualified("audio-synth/instrument", "moog-system-55")
}

/// Returns the qualified id of the default System 55 voice patch.
pub fn system55_default_patch_id() -> Symbol {
    Symbol::qualified("audio-synth/patch", "moog-system-55-synthetic-voice")
}

/// Returns the default filesystem path for a user's saved System 55 patch.
pub fn system55_user_patch_path() -> &'static str {
    SYSTEM55_USER_PATCH_PATH
}

/// Returns the repository-relative path to the System 55 synthetic-voice recipe.
pub fn system55_recipe_path() -> &'static str {
    SYSTEM55_RECIPE_PATH
}

/// Returns the symbols for all three System 55 render modes.
pub fn system55_render_mode_symbols() -> [Symbol; 3] {
    [
        System55RenderMode::Ideal.symbol(),
        System55RenderMode::Modeled.symbol(),
        System55RenderMode::Trace.symbol(),
    ]
}

/// Returns the named tap points exposed on the default System 55 voice patch.
pub fn system55_patch_points() -> [System55PatchPoint; 10] {
    SYSTEM55_PATCH_POINTS
}

/// Returns the module ids that must be present for the System 55 voice to build.
pub fn system55_required_module_ids() -> Vec<Symbol> {
    vec![
        m55_keyboard_component_id(),
        m55_vco_driver_component_id(),
        m55_vco_component_id(),
        m55_mixer_component_id(),
        m55_ladder_lpf_component_id(),
        m55_envelope_component_id(),
        m55_vca_component_id(),
        m55_fixed_filter_bank_component_id(),
        m55_sequencer_component_id(),
    ]
}

/// Returns the instrument-level port descriptors for the System 55 voice
/// (MIDI in, audio out, and optional patch and trace outputs).
pub fn system55_ports() -> Vec<ComponentPortDescriptor> {
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

/// Returns the instrument-level parameters for the System 55 voice (render mode).
pub fn system55_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "system55-render-mode"),
            "Render mode",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(system55_render_mode_symbols().to_vec(), 1),
    ]
}

/// Builds the default playable System 55 voice patch.
pub fn system55_default_patch() -> InstrumentPatch {
    system55_voice_patch(System55PatchProfile::DefaultVoice)
}

/// Builds the System 55 voice patch in its bare oscillator-stack profile.
pub fn system55_oscillator_stack_patch() -> InstrumentPatch {
    system55_voice_patch(System55PatchProfile::OscillatorStack)
}

/// Builds the System 55 voice patch in its ladder self-oscillation profile.
pub fn system55_ladder_self_oscillation_patch() -> InstrumentPatch {
    system55_voice_patch(System55PatchProfile::LadderSelfOscillation)
}

/// Builds the System 55 voice patch in its fixed-filter-bank profile.
pub fn system55_filter_bank_patch() -> InstrumentPatch {
    system55_voice_patch(System55PatchProfile::FilterBank)
}

/// Builds the System 55 voice patch in its sequencer-driven profile.
pub fn system55_sequencer_patch() -> InstrumentPatch {
    system55_voice_patch(System55PatchProfile::SequencerDriven)
}

/// Builds the System 55 voice patch in its patch-round-trip profile.
pub fn system55_patch_round_trip_patch() -> InstrumentPatch {
    system55_voice_patch(System55PatchProfile::PatchRoundTrip)
}

fn system55_voice_patch(profile: System55PatchProfile) -> InstrumentPatch {
    InstrumentPatch::new(system55_default_patch_id())
        .with_module(patch_module(
            "keyboard-1",
            m55_keyboard_component_id(),
            "keyboard",
            vec![
                PatchJack::event("key-in", false),
                PatchJack::gate("gate-in", false),
                PatchJack::cv("bend-in", false),
            ],
            vec![
                PatchJack::cv("pitch-cv-out", true),
                PatchJack::gate("s-trigger-out", true),
            ],
        ))
        .with_module(patch_module(
            "osc-driver-1",
            m55_vco_driver_component_id(),
            "oscillator-driver",
            vec![PatchJack::cv("keyboard-cv-in", true)],
            vec![PatchJack::cv("pitch-cv-out", true)],
        ))
        .with_module(vco_module("osc-1"))
        .with_module(vco_module("osc-2"))
        .with_module(vco_module("osc-3"))
        .with_module(patch_module(
            "mixer-1",
            m55_mixer_component_id(),
            "cp3a-mixer",
            vec![
                PatchJack::audio("audio-in-1", true),
                PatchJack::audio("audio-in-2", true),
                PatchJack::audio("audio-in-3", true),
            ],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "ladder-1",
            m55_ladder_lpf_component_id(),
            "ladder-filter",
            vec![
                PatchJack::audio("audio-in", true),
                PatchJack::cv("cutoff-cv-in", false),
                PatchJack::cv("resonance-cv-in", false),
            ],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "envelope-1",
            m55_envelope_component_id(),
            "envelope",
            vec![PatchJack::gate("s-trigger-in", true)],
            vec![PatchJack::cv("envelope-cv-out", true)],
        ))
        .with_module(patch_module(
            "vca-1",
            m55_vca_component_id(),
            "vca",
            vec![
                PatchJack::audio("audio-in", true),
                PatchJack::cv("gain-cv-in", true),
            ],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "filter-bank-1",
            m55_fixed_filter_bank_component_id(),
            "fixed-filter-bank",
            vec![PatchJack::audio("audio-in", true)],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "sequencer-1",
            m55_sequencer_component_id(),
            "sequencer",
            vec![PatchJack::gate("s-trigger-in", true)],
            vec![
                PatchJack::cv("cv-out", true),
                PatchJack::gate("s-trigger-out", true),
            ],
        ))
        .with_cord(cord(
            "keyboard-1",
            "pitch-cv-out",
            "osc-driver-1",
            "keyboard-cv-in",
        ))
        .with_cord(cord("osc-driver-1", "pitch-cv-out", "osc-1", "pitch-cv-in"))
        .with_cord(cord("osc-driver-1", "pitch-cv-out", "osc-2", "pitch-cv-in"))
        .with_cord(cord("osc-driver-1", "pitch-cv-out", "osc-3", "pitch-cv-in"))
        .with_cord(cord("osc-1", "audio-out", "mixer-1", "audio-in-1"))
        .with_cord(cord("osc-2", "audio-out", "mixer-1", "audio-in-2"))
        .with_cord(cord("osc-3", "audio-out", "mixer-1", "audio-in-3"))
        .with_cord(cord("mixer-1", "audio-out", "ladder-1", "audio-in"))
        .with_cord(cord(
            "keyboard-1",
            "s-trigger-out",
            "envelope-1",
            "s-trigger-in",
        ))
        .with_cord(cord(
            "envelope-1",
            "envelope-cv-out",
            "ladder-1",
            "cutoff-cv-in",
        ))
        .with_cord(cord("ladder-1", "audio-out", "vca-1", "audio-in"))
        .with_cord(cord("envelope-1", "envelope-cv-out", "vca-1", "gain-cv-in"))
        .with_cord(cord("vca-1", "audio-out", "filter-bank-1", "audio-in"))
        .with_cord(cord(
            "sequencer-1",
            "cv-out",
            "osc-driver-1",
            "keyboard-cv-in",
        ))
        .with_setting(profile_key(), Expr::String(profile.as_str().to_owned()))
        .with_setting(
            Symbol::qualified("audio-synth/system55", "user-patch-path"),
            Expr::String(SYSTEM55_USER_PATCH_PATH.to_owned()),
        )
        .with_setting(
            Symbol::qualified("audio-synth/system55", "patch-points"),
            Expr::Vector(
                SYSTEM55_PATCH_POINTS
                    .iter()
                    .map(|point| Expr::String(point.name.to_owned()))
                    .collect(),
            ),
        )
}

fn patch_module(
    id: &'static str,
    kind: Symbol,
    role: &'static str,
    inputs: Vec<PatchJack>,
    outputs: Vec<PatchJack>,
) -> PatchModule {
    let mut module = PatchModule::new(Symbol::new(id), kind).with_setting(
        Symbol::qualified("audio-synth/system55", "role"),
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

fn vco_module(id: &'static str) -> PatchModule {
    patch_module(
        id,
        m55_vco_component_id(),
        "oscillator",
        vec![PatchJack::cv("pitch-cv-in", true)],
        vec![PatchJack::audio("audio-out", true)],
    )
}

fn cord(
    from_module: &'static str,
    from_jack: &'static str,
    to_module: &'static str,
    to_jack: &'static str,
) -> PatchCord {
    PatchCord::new(
        PatchEndpoint::new(from_module, from_jack),
        PatchEndpoint::new(to_module, to_jack),
    )
}

fn profile_key() -> Symbol {
    Symbol::qualified("audio-synth/system55", "profile")
}
