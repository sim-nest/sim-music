use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::ps3300::{
    PS3300_KEY_COUNT, PS3300_SECTION_COUNT, PS3300_TOTAL_KEY_CELLS, Ps3300Section,
    ps3_external_processor_component_id, ps3_keyboard_component_id,
    ps3_modulation_generator_component_id, ps3_output_mixer_component_id,
    ps3_per_key_cell_component_id, ps3_pin_matrix_component_id, ps3_poly_array_component_id,
    ps3_resonator_component_id, ps3_sample_hold_component_id, ps3_section_generator_component_id,
    ps3_tonegen_component_id, ps3300_default_pin_matrix_routes, ps3300_validate_pin_matrix_routes,
};
use crate::{
    ComponentParamDescriptor, ComponentParamUnit, ComponentPortDescriptor, ComponentPortDirection,
    ComponentPortMedia, InstrumentPatch, PatchCord, PatchEndpoint, PatchJack, PatchModule,
};

/// User-facing filesystem path where the default PS-3300 patch is stored.
pub const PS3300_USER_PATCH_PATH: &str =
    "$HOME/.local/share/sim/ps3300/synthetic-polyphonic.patch.siml";
/// Repository-relative path to the default PS-3300 patch recipe.
pub const PS3300_RECIPE_PATH: &str =
    "crates/sim-lib-music-synth/recipes/ps3300/synthetic-polyphonic-patch/recipe.toml";

/// Fidelity mode used when rendering the PS-3300.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ps3300RenderMode {
    /// Clean algorithmic render with no analog modeling.
    Ideal,
    /// Analog-modeled render with nonlinear saturation.
    Modeled,
    /// Modeled render that also captures component trace frames.
    Trace,
}

impl Ps3300RenderMode {
    /// Returns the stable kebab-case identifier string for this render mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ideal => "ideal",
            Self::Modeled => "modeled",
            Self::Trace => "trace",
        }
    }

    /// Returns the qualified `audio-synth/ps3300-render-mode` symbol for this mode.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-render-mode", self.as_str())
    }
}

/// Pre-wired PS-3300 patch variant selecting which voicing the wrapper renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ps3300PatchProfile {
    /// Single voice cell.
    OneCell,
    /// One section playing a chord.
    OneSectionChord,
    /// Resonator formant sweep.
    ResonatorSweep,
    /// All three sections stacked.
    ThreeSectionStack,
    /// Full default polyphonic patch.
    DefaultPolyphonic,
    /// Default patch used for serialization round-trip checks.
    PatchRoundTrip,
}

impl Ps3300PatchProfile {
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
            .unwrap_or(Self::DefaultPolyphonic)
    }

    fn from_str(value: &str) -> Self {
        match value {
            "one-cell" => Self::OneCell,
            "one-section-chord" => Self::OneSectionChord,
            "resonator-sweep" => Self::ResonatorSweep,
            "three-section-stack" => Self::ThreeSectionStack,
            "patch-round-trip" => Self::PatchRoundTrip,
            _ => Self::DefaultPolyphonic,
        }
    }

    /// Returns the stable kebab-case identifier string for this profile.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneCell => "one-cell",
            Self::OneSectionChord => "one-section-chord",
            Self::ResonatorSweep => "resonator-sweep",
            Self::ThreeSectionStack => "three-section-stack",
            Self::DefaultPolyphonic => "default-polyphonic",
            Self::PatchRoundTrip => "patch-round-trip",
        }
    }
}

/// Summary of the PS-3300's polyphonic capacity across its sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ps3300PolyphonySummary {
    /// Number of sound sections.
    pub section_count: usize,
    /// Number of keys per section.
    pub keys_per_section: usize,
    /// Total per-key voice cells across all sections.
    pub total_key_cells: usize,
}

impl Ps3300PolyphonySummary {
    /// Encodes the polyphony summary as a kernel `Expr` map for patch settings.
    pub fn to_expr(self) -> Expr {
        Expr::Map(vec![
            (field("section-count"), number_usize(self.section_count)),
            (
                field("keys-per-section"),
                number_usize(self.keys_per_section),
            ),
            (field("total-key-cells"), number_usize(self.total_key_cells)),
        ])
    }
}

/// Returns the patch id for the default PS-3300 synthetic-polyphonic patch.
pub fn ps3300_default_patch_id() -> Symbol {
    Symbol::qualified("audio-synth/patch", "korg-ps-3300-synthetic-polyphonic")
}

/// Returns the user-facing path where the default PS-3300 patch is stored.
pub fn ps3300_user_patch_path() -> &'static str {
    PS3300_USER_PATCH_PATH
}

/// Returns the repository-relative path to the default PS-3300 patch recipe.
pub fn ps3300_recipe_path() -> &'static str {
    PS3300_RECIPE_PATH
}

/// Returns the qualified symbols of all three PS-3300 render modes.
pub fn ps3300_render_mode_symbols() -> [Symbol; 3] {
    [
        Ps3300RenderMode::Ideal.symbol(),
        Ps3300RenderMode::Modeled.symbol(),
        Ps3300RenderMode::Trace.symbol(),
    ]
}

/// Returns the PS-3300 polyphony summary derived from the section and key counts.
pub fn ps3300_polyphony_summary() -> Ps3300PolyphonySummary {
    Ps3300PolyphonySummary {
        section_count: PS3300_SECTION_COUNT,
        keys_per_section: PS3300_KEY_COUNT,
        total_key_cells: PS3300_TOTAL_KEY_CELLS,
    }
}

/// Returns the PS-3300 signal-flow edges as `from->to` strings.
pub fn ps3300_section_graph() -> Vec<String> {
    [
        "keyboard->pin-matrix",
        "modulation-generator->pin-matrix",
        "sample-hold->pin-matrix",
        "external-processor->pin-matrix",
        "pin-matrix->section-a",
        "pin-matrix->section-b",
        "pin-matrix->section-c",
        "sections->triple-resonator",
        "sections+resonator->output-mixer",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// Returns the component ids that must be registered for the PS-3300 to build.
pub fn ps3300_required_module_ids() -> Vec<Symbol> {
    vec![
        ps3_keyboard_component_id(),
        ps3_tonegen_component_id(),
        ps3_per_key_cell_component_id(),
        ps3_poly_array_component_id(),
        ps3_resonator_component_id(),
        ps3_pin_matrix_component_id(),
        ps3_modulation_generator_component_id(),
        ps3_sample_hold_component_id(),
        ps3_external_processor_component_id(),
        ps3_section_generator_component_id(),
        ps3_output_mixer_component_id(),
    ]
}

/// Returns the PS-3300 instrument's port descriptors (MIDI in, audio/patch/trace out).
pub fn ps3300_ports() -> Vec<ComponentPortDescriptor> {
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

/// Returns the PS-3300 instrument's parameter descriptors (the render-mode enum).
pub fn ps3300_params() -> Vec<ComponentParamDescriptor> {
    vec![
        ComponentParamDescriptor::new(
            Symbol::qualified("audio-synth/param", "ps3300-render-mode"),
            "Render mode",
            ComponentParamUnit::Unitless,
        )
        .with_enum_values(ps3300_render_mode_symbols().to_vec(), 1),
    ]
}

/// Builds the full default polyphonic PS-3300 patch.
pub fn ps3300_default_patch() -> InstrumentPatch {
    ps3300_patch(Ps3300PatchProfile::DefaultPolyphonic)
}

/// Builds the single-cell PS-3300 patch profile.
pub fn ps3300_one_cell_patch() -> InstrumentPatch {
    ps3300_patch(Ps3300PatchProfile::OneCell)
}

/// Builds the one-section-chord PS-3300 patch profile.
pub fn ps3300_one_section_chord_patch() -> InstrumentPatch {
    ps3300_patch(Ps3300PatchProfile::OneSectionChord)
}

/// Builds the resonator-sweep PS-3300 patch profile.
pub fn ps3300_resonator_sweep_patch() -> InstrumentPatch {
    ps3300_patch(Ps3300PatchProfile::ResonatorSweep)
}

/// Builds the three-section-stack PS-3300 patch profile.
pub fn ps3300_three_section_stack_patch() -> InstrumentPatch {
    ps3300_patch(Ps3300PatchProfile::ThreeSectionStack)
}

/// Builds the patch-round-trip PS-3300 patch profile.
pub fn ps3300_patch_round_trip_patch() -> InstrumentPatch {
    ps3300_patch(Ps3300PatchProfile::PatchRoundTrip)
}

fn ps3300_patch(profile: Ps3300PatchProfile) -> InstrumentPatch {
    let routes = ps3300_default_pin_matrix_routes();
    ps3300_validate_pin_matrix_routes(&routes).expect("default PS-3300 pin routes are valid");
    InstrumentPatch::new(patch_id(profile))
        .with_module(patch_module(
            "keyboard-1",
            ps3_keyboard_component_id(),
            "keyboard-controller",
            vec![PatchJack::event("key-in", false)],
            vec![
                PatchJack::cv("pitch-cv-out", true),
                PatchJack::gate("gate-out", true),
                PatchJack::gate("trigger-out", false),
            ],
        ))
        .with_module(patch_module(
            "modulation-1",
            ps3_modulation_generator_component_id(),
            "modulation-generator",
            vec![PatchJack::cv("rate-cv-in", false)],
            vec![PatchJack::cv("cv-out", true)],
        ))
        .with_module(patch_module(
            "sample-hold-1",
            ps3_sample_hold_component_id(),
            "sample-hold",
            vec![
                PatchJack::cv("signal-in", true),
                PatchJack::gate("trigger-in", true),
            ],
            vec![PatchJack::cv("held-out", true)],
        ))
        .with_module(patch_module(
            "external-1",
            ps3_external_processor_component_id(),
            "external-processor",
            vec![
                PatchJack::audio("audio-in", false),
                PatchJack::cv("cv-in", false),
            ],
            vec![
                PatchJack::audio("audio-out", false),
                PatchJack::cv("cv-out", false),
                PatchJack::gate("gate-out", false),
            ],
        ))
        .with_module(patch_module(
            "pin-matrix-1",
            ps3_pin_matrix_component_id(),
            "pin-matrix",
            vec![
                PatchJack::cv("keyboard-pitch-cv", true),
                PatchJack::gate("keyboard-gate", true),
                PatchJack::cv("modulation-cv", false),
                PatchJack::cv("sample-hold-cv", false),
                PatchJack::cv("external-cv", false),
                PatchJack::gate("external-gate", false),
            ],
            vec![
                PatchJack::cv("section-a-pitch-cv", true),
                PatchJack::cv("section-b-pitch-cv", true),
                PatchJack::cv("section-c-pitch-cv", true),
                PatchJack::gate("section-a-gate", true),
                PatchJack::gate("section-b-gate", true),
                PatchJack::gate("section-c-gate", true),
                PatchJack::audio("resonator-audio-in", true),
                PatchJack::cv("resonator-formant-cv", false),
            ],
        ))
        .with_module(section_patch_module("section-a", Ps3300Section::A))
        .with_module(section_patch_module("section-b", Ps3300Section::B))
        .with_module(section_patch_module("section-c", Ps3300Section::C))
        .with_module(patch_module(
            "resonator-1",
            ps3_resonator_component_id(),
            "triple-resonator",
            vec![
                PatchJack::audio("audio-in", true),
                PatchJack::cv("formant-cv-in", false),
            ],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_module(patch_module(
            "output-mixer-1",
            ps3_output_mixer_component_id(),
            "three-section-summer",
            vec![
                PatchJack::audio("section-a-audio", true),
                PatchJack::audio("section-b-audio", true),
                PatchJack::audio("section-c-audio", true),
                PatchJack::audio("resonator-audio", true),
            ],
            vec![PatchJack::audio("audio-out", true)],
        ))
        .with_cord(cord(
            "keyboard-1",
            "pitch-cv-out",
            "pin-matrix-1",
            "keyboard-pitch-cv",
        ))
        .with_cord(cord(
            "keyboard-1",
            "gate-out",
            "pin-matrix-1",
            "keyboard-gate",
        ))
        .with_cord(cord(
            "modulation-1",
            "cv-out",
            "pin-matrix-1",
            "modulation-cv",
        ))
        .with_cord(cord(
            "sample-hold-1",
            "held-out",
            "pin-matrix-1",
            "sample-hold-cv",
        ))
        .with_cord(cord("external-1", "cv-out", "pin-matrix-1", "external-cv"))
        .with_cord(cord(
            "external-1",
            "gate-out",
            "pin-matrix-1",
            "external-gate",
        ))
        .with_cord(cord(
            "pin-matrix-1",
            "section-a-pitch-cv",
            "section-a",
            "pitch-cv-in",
        ))
        .with_cord(cord(
            "pin-matrix-1",
            "section-b-pitch-cv",
            "section-b",
            "pitch-cv-in",
        ))
        .with_cord(cord(
            "pin-matrix-1",
            "section-c-pitch-cv",
            "section-c",
            "pitch-cv-in",
        ))
        .with_cord(cord(
            "pin-matrix-1",
            "section-a-gate",
            "section-a",
            "gate-in",
        ))
        .with_cord(cord(
            "pin-matrix-1",
            "section-b-gate",
            "section-b",
            "gate-in",
        ))
        .with_cord(cord(
            "pin-matrix-1",
            "section-c-gate",
            "section-c",
            "gate-in",
        ))
        .with_cord(cord("section-a", "audio-out", "resonator-1", "audio-in"))
        .with_cord(cord(
            "resonator-1",
            "audio-out",
            "output-mixer-1",
            "resonator-audio",
        ))
        .with_cord(cord(
            "section-a",
            "audio-out",
            "output-mixer-1",
            "section-a-audio",
        ))
        .with_cord(cord(
            "section-b",
            "audio-out",
            "output-mixer-1",
            "section-b-audio",
        ))
        .with_cord(cord(
            "section-c",
            "audio-out",
            "output-mixer-1",
            "section-c-audio",
        ))
        .with_setting(profile_key(), Expr::String(profile.as_str().to_owned()))
        .with_setting(
            setting_key("user-patch-path"),
            Expr::String(PS3300_USER_PATCH_PATH.to_owned()),
        )
        .with_setting(
            setting_key("polyphony-summary"),
            ps3300_polyphony_summary().to_expr(),
        )
        .with_setting(
            setting_key("section-graph"),
            Expr::Vector(
                ps3300_section_graph()
                    .into_iter()
                    .map(Expr::String)
                    .collect(),
            ),
        )
}

fn patch_id(profile: Ps3300PatchProfile) -> Symbol {
    match profile {
        Ps3300PatchProfile::DefaultPolyphonic | Ps3300PatchProfile::PatchRoundTrip => {
            ps3300_default_patch_id()
        }
        _ => Symbol::qualified(
            "audio-synth/patch",
            format!("korg-ps-3300-{}", profile.as_str()),
        ),
    }
}

fn section_patch_module(id: &'static str, section: Ps3300Section) -> PatchModule {
    patch_module(
        id,
        ps3_section_generator_component_id(),
        "section-generator",
        vec![
            PatchJack::cv("pitch-cv-in", true),
            PatchJack::gate("gate-in", true),
            PatchJack::cv("modulation-cv-in", false),
        ],
        vec![PatchJack::audio("audio-out", true)],
    )
    .with_setting(setting_key("section"), Expr::Symbol(section.symbol()))
}

fn patch_module(
    id: &'static str,
    kind: Symbol,
    role: &'static str,
    inputs: Vec<PatchJack>,
    outputs: Vec<PatchJack>,
) -> PatchModule {
    let mut module = PatchModule::new(Symbol::new(id), kind)
        .with_setting(setting_key("role"), Expr::String(role.to_owned()));
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
        PatchEndpoint::new(from_module, from_jack),
        PatchEndpoint::new(to_module, to_jack),
    )
}

fn profile_key() -> Symbol {
    setting_key("wrapper-profile")
}

fn setting_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300", name)
}

fn field(name: &'static str) -> Expr {
    Expr::Symbol(setting_key(name))
}

fn number_usize(value: usize) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}
