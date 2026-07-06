use sim_kernel::{Expr, NumberLiteral, Result, Symbol};

use crate::{GateConvention, PolyphonicArray, PolyphonicSectionSetting, VoltsPerOctave};

pub use crate::modules::ps3_cell::{
    PS3300_VOICE_CELL_FIXTURE_NAMES, Ps3300NoteCell, Ps3300NoteCellFrame, Ps3300NoteCellSettings,
    Ps3300PerNoteEnvelopeSettings, Ps3300PerNoteVcaSettings, Ps3300PerNoteVcfSettings,
    Ps3300PolyArray, Ps3300PolyArrayFrame, Ps3300PolyArraySettings, ps3_per_key_cell_component_id,
    ps3_per_key_cell_params, ps3_per_key_cell_ports, ps3_poly_array_component_id,
    ps3_poly_array_params, ps3_poly_array_ports, ps3300_voice_cell_fixture_names,
    ps3300_voice_cell_module_ids,
};
pub use crate::modules::ps3_keyboard::{
    Ps3300KeyboardController, Ps3300KeyboardFrame, Ps3300KeyboardGateMapping,
    Ps3300KeyboardSettings, ps3_keyboard_component_id, ps3_keyboard_params, ps3_keyboard_ports,
    ps3300_keyboard_gate_mapping,
};
pub use crate::modules::ps3_matrix::{
    Ps3300PinMatrix, Ps3300PinMatrixFormat, Ps3300PinMatrixFrame, Ps3300PinMatrixInputs,
    ps3_pin_matrix_component_id, ps3_pin_matrix_params, ps3_pin_matrix_ports,
    ps3300_pin_matrix_format, ps3300_pin_matrix_pair_is_legal, ps3300_pin_matrix_source_names,
    ps3300_pin_matrix_target_names,
};
pub use crate::modules::ps3_modulation::{
    PS3300_MODULATION_FIXTURE_NAMES, Ps3300ExternalProcessor, Ps3300ExternalProcessorFrame,
    Ps3300ExternalProcessorSettings, Ps3300ModulationFrame, Ps3300ModulationGenerator,
    Ps3300ModulationGeneratorSettings, Ps3300ModulationWaveform, Ps3300SampleHold,
    Ps3300SampleHoldFrame, Ps3300SampleHoldSettings, ps3_external_processor_component_id,
    ps3_external_processor_params, ps3_external_processor_ports,
    ps3_modulation_generator_component_id, ps3_modulation_generator_params,
    ps3_modulation_generator_ports, ps3_sample_hold_component_id, ps3_sample_hold_params,
    ps3_sample_hold_ports, ps3300_modulation_fixture_names, ps3300_modulation_module_ids,
};
pub use crate::modules::ps3_noise::{
    Ps3300Noise, Ps3300NoiseColor, Ps3300NoiseFrame, Ps3300NoiseSettings, ps3_noise_component_id,
    ps3_noise_params, ps3_noise_ports,
};
pub use crate::modules::ps3_resonator::{
    Ps3300ResonatorBandSettings, Ps3300ResonatorFrame, Ps3300ResonatorMode, Ps3300TripleResonator,
    Ps3300TripleResonatorSettings, ps3_resonator_component_id, ps3_resonator_params,
    ps3_resonator_ports,
};
pub use crate::modules::ps3_section::{
    PS3300_SECTION_FIXTURE_NAMES, Ps3300SectionFrame, Ps3300SectionGenerator,
    Ps3300SectionGeneratorSettings, Ps3300ThreeSectionSummer, Ps3300ThreeSectionSummerFrame,
    Ps3300ThreeSectionSummerSettings, ps3_output_mixer_component_id, ps3_output_mixer_params,
    ps3_output_mixer_ports, ps3_section_generator_component_id, ps3_section_generator_params,
    ps3_section_generator_ports, ps3300_section_fixture_names, ps3300_section_module_ids,
};
pub use crate::modules::ps3_tonegen::{
    PS3300_FOOTAGES, PS3300_MASTER_OSCILLATOR_COUNT, PS3300_TONE_SOURCE_FIXTURE_NAMES,
    Ps3300AliasedFrequency, Ps3300AliasingPolicy, Ps3300DividerPlan, Ps3300Footage,
    Ps3300FootageLevels, Ps3300ToneFrame, Ps3300ToneSource, Ps3300ToneSourceSettings,
    Ps3300ToneWaveform, ps3_tonegen_component_id, ps3_tonegen_params, ps3_tonegen_ports,
    ps3300_pitch_coverage, ps3300_tone_divider_plan, ps3300_tone_source_fixture_names,
    ps3300_tone_source_module_ids,
};

mod patch;
pub use patch::{ps3300_per_key_cell_patch, ps3300_scaffold_patch};

/// Repository-relative path to this PS-3300 instrument model source file.
pub const PS3300_SOURCE_PATH: &str = "crates/sim-lib-music-synth/src/ps3300.rs";
/// Repository-relative path to the PS-3300 recipe book manifest.
pub const PS3300_RECIPE_BOOK_PATH: &str = "crates/sim-lib-music-synth/recipes/ps3300/book.toml";
/// Repository-relative path to the PS-3300 recipe chapter manifest.
pub const PS3300_RECIPE_CHAPTER_PATH: &str =
    "crates/sim-lib-music-synth/recipes/ps3300/chapter.toml";
/// Number of independent sound-generating sections (A, B, C) on the PS-3300.
pub const PS3300_SECTION_COUNT: usize = 3;
/// Number of keys covered by each section's polyphonic key array.
pub const PS3300_KEY_COUNT: usize = 48;
/// Total per-key cells across all sections (sections times keys per section).
pub const PS3300_TOTAL_KEY_CELLS: usize = PS3300_SECTION_COUNT * PS3300_KEY_COUNT;
/// Names of the patch model types that make up the PS-3300 scaffold patch.
pub const PS3300_PATCH_MODEL_NAMES: [&str; 7] = [
    "Ps3300ModuleDescriptor",
    "Ps3300ModuleRole",
    "Ps3300Section",
    "Ps3300KeyboardAssignment",
    "Ps3300ResonatorSettings",
    "Ps3300PinMatrixRoute",
    "PolyphonicArray",
];

const PS3300_MODULE_DESCRIPTORS: [Ps3300ModuleDescriptor; 10] = [
    Ps3300ModuleDescriptor::new(
        "ps3-keyboard-controller",
        "PS-3300 Keyboard Controller",
        Ps3300ModuleRole::Keyboard,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-key-assigner",
        "PS-3300 Key Assigner",
        Ps3300ModuleRole::Keyboard,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-section-generator",
        "PS-3300 Section Generator",
        Ps3300ModuleRole::Section,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-tone-source",
        "PS-3300 Tone Source",
        Ps3300ModuleRole::ToneSource,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-noise-source",
        "PS-3300 Noise Source",
        Ps3300ModuleRole::Noise,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-per-key-cell",
        "PS-3300 Per-Key Cell",
        Ps3300ModuleRole::PerKeyCell,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-resonator-bank",
        "PS-3300 Resonator Bank",
        Ps3300ModuleRole::Resonator,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-pin-matrix",
        "PS-3300 Pin Matrix",
        Ps3300ModuleRole::PinMatrix,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-modulation-generator",
        "PS-3300 Modulation Generator",
        Ps3300ModuleRole::Modulation,
    ),
    Ps3300ModuleDescriptor::new(
        "ps3-output-mixer",
        "PS-3300 Output Mixer",
        Ps3300ModuleRole::Mixer,
    ),
];

/// Functional role a PS-3300 module plays within the instrument graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ps3300ModuleRole {
    /// Keyboard controller and key-assigner role.
    Keyboard,
    /// Master tone source (oscillator/divider) role.
    ToneSource,
    /// Noise source role.
    Noise,
    /// Per-key voice cell role.
    PerKeyCell,
    /// Section generator role (one of the three sound sections).
    Section,
    /// Triple resonator (formant filter bank) role.
    Resonator,
    /// Pin-matrix patch routing role.
    PinMatrix,
    /// Modulation generator role.
    Modulation,
    /// Output mixer role.
    Mixer,
}

impl Ps3300ModuleRole {
    /// Returns the stable kebab-case identifier string for this role.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::ToneSource => "tone-source",
            Self::Noise => "noise",
            Self::PerKeyCell => "per-key-cell",
            Self::Section => "section",
            Self::Resonator => "resonator",
            Self::PinMatrix => "pin-matrix",
            Self::Modulation => "modulation",
            Self::Mixer => "mixer",
        }
    }

    /// Returns the qualified `audio-synth/ps3300-role` symbol for this role.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-role", self.as_str())
    }
}

/// Static description of one PS-3300 module: its id name, display label, and role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ModuleDescriptor {
    /// Stable id-name fragment used to build the module's qualified symbol.
    pub id_name: &'static str,
    /// Human-readable display label for the module.
    pub label: &'static str,
    /// Functional role this module fills.
    pub role: Ps3300ModuleRole,
}

impl Ps3300ModuleDescriptor {
    const fn new(id_name: &'static str, label: &'static str, role: Ps3300ModuleRole) -> Self {
        Self {
            id_name,
            label,
            role,
        }
    }

    /// Returns the qualified `audio-synth/module` symbol built from [`id_name`](Self::id_name).
    pub fn id(self) -> Symbol {
        ps3300_module_id(self.id_name)
    }
}

/// One of the PS-3300's three independent sound sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ps3300Section {
    /// Section A.
    A,
    /// Section B.
    B,
    /// Section C.
    C,
}

impl Ps3300Section {
    /// Returns the stable kebab-case identifier string for this section.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "section-a",
            Self::B => "section-b",
            Self::C => "section-c",
        }
    }

    /// Returns the qualified `audio-synth/ps3300-section` symbol for this section.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("audio-synth/ps3300-section", self.as_str())
    }
}

/// Keyboard-to-voice assignment describing the PS-3300's key range and polyphony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ps3300KeyboardAssignment {
    /// MIDI note number of the lowest playable key.
    pub first_midi_key: u8,
    /// Number of contiguous keys covered by the assignment.
    pub key_count: usize,
    /// Whether every key has its own voice cell (full polyphony).
    pub full_polyphonic: bool,
}

impl Default for Ps3300KeyboardAssignment {
    fn default() -> Self {
        Self {
            first_midi_key: 36,
            key_count: PS3300_KEY_COUNT,
            full_polyphonic: true,
        }
    }
}

impl Ps3300KeyboardAssignment {
    /// Encodes the assignment as a kernel `Expr` map for patch raw views.
    pub fn to_expr(self) -> Expr {
        Expr::Map(vec![
            (
                field("first-midi-key"),
                number_usize(self.first_midi_key.into()),
            ),
            (field("key-count"), number_usize(self.key_count)),
            (field("full-polyphonic"), Expr::Bool(self.full_polyphonic)),
        ])
    }
}

/// Center frequencies and emphasis for the PS-3300 triple resonator bank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ps3300ResonatorSettings {
    /// Low-band center frequency in hertz.
    pub low_hz: f32,
    /// Mid-band center frequency in hertz.
    pub mid_hz: f32,
    /// High-band center frequency in hertz.
    pub high_hz: f32,
    /// Resonance emphasis (0.0 flat to 1.0 sharp) applied across the bands.
    pub emphasis: f32,
}

impl Default for Ps3300ResonatorSettings {
    fn default() -> Self {
        Self {
            low_hz: 720.0,
            mid_hz: 1_440.0,
            high_hz: 2_880.0,
            emphasis: 0.45,
        }
    }
}

impl Ps3300ResonatorSettings {
    /// Encodes the resonator settings as a kernel `Expr` map for patches.
    pub fn to_expr(self) -> Expr {
        Expr::Map(vec![
            (field("low-hz"), number_f32(self.low_hz)),
            (field("mid-hz"), number_f32(self.mid_hz)),
            (field("high-hz"), number_f32(self.high_hz)),
            (field("emphasis"), number_f32(self.emphasis)),
        ])
    }
}

/// One routing connection in the PS-3300 pin matrix: a source, a target, and a depth.
#[derive(Clone, Debug, PartialEq)]
pub struct Ps3300PinMatrixRoute {
    /// Name of the matrix source jack feeding the route.
    pub source: String,
    /// Name of the matrix target jack receiving the route.
    pub target: String,
    /// Routing amount (signal depth) applied from source to target.
    pub amount: f32,
}

impl Ps3300PinMatrixRoute {
    /// Builds a route from a source name, target name, and routing amount.
    pub fn new(source: impl Into<String>, target: impl Into<String>, amount: f32) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            amount,
        }
    }

    /// Encodes the route as a kernel `Expr` map with symbol source/target and amount.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("source"),
                Expr::Symbol(Symbol::new(self.source.clone())),
            ),
            (
                field("target"),
                Expr::Symbol(Symbol::new(self.target.clone())),
            ),
            (field("amount"), number_f32(self.amount)),
        ])
    }
}

/// Returns the registry component id for the PS-3300 instrument.
pub fn ps3300_component_id() -> Symbol {
    crate::registry::ps_3300_component_id()
}

/// Returns the patch id for the PS-3300 scaffold (topology-only) patch.
pub fn ps3300_scaffold_patch_id() -> Symbol {
    Symbol::qualified("audio-synth/patch", "korg-ps-3300-scaffold")
}

/// Returns the patch id for the PS-3300 per-key-cell patch.
pub fn ps3300_per_key_cell_patch_id() -> Symbol {
    Symbol::qualified("audio-synth/patch", "korg-ps-3300-per-key-cells")
}

/// Returns the full list of PS-3300 module descriptors.
pub fn ps3300_module_descriptors() -> Vec<Ps3300ModuleDescriptor> {
    PS3300_MODULE_DESCRIPTORS.to_vec()
}

/// Returns the qualified symbol ids of every PS-3300 module.
pub fn ps3300_module_ids() -> Vec<Symbol> {
    ps3300_module_descriptors()
        .into_iter()
        .map(Ps3300ModuleDescriptor::id)
        .collect()
}

/// Returns the patch model type names used by the scaffold patch.
pub fn ps3300_patch_model_names() -> [&'static str; 7] {
    PS3300_PATCH_MODEL_NAMES
}

/// Returns the three PS-3300 sections in A, B, C order.
pub fn ps3300_sections() -> [Ps3300Section; 3] {
    [Ps3300Section::A, Ps3300Section::B, Ps3300Section::C]
}

/// Returns the default PS-3300 keyboard-to-voice assignment.
pub fn ps3300_keyboard_assignment() -> Ps3300KeyboardAssignment {
    Ps3300KeyboardAssignment::default()
}

/// Returns the default PS-3300 resonator settings.
pub fn ps3300_resonator_settings() -> Ps3300ResonatorSettings {
    Ps3300ResonatorSettings::default()
}

/// Returns the default pin-matrix routes that wire the scaffold patch.
pub fn ps3300_default_pin_matrix_routes() -> Vec<Ps3300PinMatrixRoute> {
    vec![
        Ps3300PinMatrixRoute::new("keyboard-pitch-cv", "section-a-pitch-cv", 1.0),
        Ps3300PinMatrixRoute::new("keyboard-pitch-cv", "section-b-pitch-cv", 1.0),
        Ps3300PinMatrixRoute::new("keyboard-pitch-cv", "section-c-pitch-cv", 1.0),
        Ps3300PinMatrixRoute::new("keyboard-gate", "section-a-gate", 1.0),
        Ps3300PinMatrixRoute::new("keyboard-gate", "section-b-gate", 1.0),
        Ps3300PinMatrixRoute::new("keyboard-gate", "section-c-gate", 1.0),
        Ps3300PinMatrixRoute::new("section-a-audio", "resonator-audio-in", 0.8),
        Ps3300PinMatrixRoute::new("section-b-audio", "resonator-audio-in", 0.8),
        Ps3300PinMatrixRoute::new("section-c-audio", "resonator-audio-in", 0.8),
        Ps3300PinMatrixRoute::new("resonator-audio", "output-mixer-audio-in", 1.0),
    ]
}

/// Validates that every route's source/target pair is legal in the pin matrix.
pub fn ps3300_validate_pin_matrix_routes(routes: &[Ps3300PinMatrixRoute]) -> Result<()> {
    crate::modules::ps3_matrix::validate_pin_matrix_routes(routes)
}

/// Returns one polyphonic key array per section (A, B, C).
pub fn ps3300_section_polyphonic_arrays() -> Vec<PolyphonicArray> {
    ps3300_sections()
        .into_iter()
        .map(ps3300_polyphonic_array)
        .collect()
}

/// Builds the polyphonic key array for one section, with level and resonator-send settings.
pub fn ps3300_polyphonic_array(section: Ps3300Section) -> PolyphonicArray {
    PolyphonicArray::new(
        Symbol::qualified("audio-synth/ps3300-poly-array", section.as_str()),
        PS3300_KEY_COUNT,
        VoltsPerOctave::new(ps3300_keyboard_assignment().first_midi_key, 1.0),
        GateConvention::voltage_gate(),
    )
    .with_section_setting(PolyphonicSectionSetting::new(
        section.symbol(),
        Symbol::new("section-level"),
        number_f32(0.82),
    ))
    .with_section_setting(PolyphonicSectionSetting::new(
        section.symbol(),
        Symbol::new("resonator-send"),
        number_f32(0.8),
    ))
}

fn ps3300_module_id(id_name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/module", id_name)
}

fn setting_key(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300", name)
}

fn field(name: &'static str) -> Expr {
    Expr::Symbol(setting_key(name))
}

fn number_f32(value: f32) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "f64"),
        canonical: value.to_string(),
    })
}

fn number_usize(value: usize) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}
