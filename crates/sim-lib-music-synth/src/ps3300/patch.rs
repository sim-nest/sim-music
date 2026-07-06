use sim_kernel::{Expr, Symbol};

use super::{
    PS3300_KEY_COUNT, PS3300_PATCH_MODEL_NAMES, PS3300_SECTION_COUNT, PS3300_TOTAL_KEY_CELLS,
    Ps3300ModuleRole, Ps3300PinMatrixRoute, Ps3300Section, number_usize,
    ps3300_default_pin_matrix_routes, ps3300_keyboard_assignment, ps3300_module_id,
    ps3300_module_ids, ps3300_per_key_cell_patch_id, ps3300_polyphonic_array,
    ps3300_resonator_settings, ps3300_scaffold_patch_id, ps3300_section_polyphonic_arrays,
    ps3300_sections, ps3300_validate_pin_matrix_routes, setting_key,
};
use crate::{InstrumentPatch, PatchCord, PatchEndpoint, PatchJack, PatchModule, PatchRawView};

/// Builds the PS-3300 scaffold patch: full module topology, cords, and raw view.
pub fn ps3300_scaffold_patch() -> InstrumentPatch {
    let routes = ps3300_default_pin_matrix_routes();
    ps3300_validate_pin_matrix_routes(&routes).expect("default PS-3300 pin routes are valid");
    let raw_view = ps3300_raw_view(&routes);
    let mut patch = InstrumentPatch::new(ps3300_scaffold_patch_id())
        .with_raw_view(raw_view)
        .with_setting(
            setting_key("module-ids"),
            Expr::Vector(ps3300_module_ids().into_iter().map(Expr::Symbol).collect()),
        )
        .with_setting(
            setting_key("patch-model-names"),
            Expr::Vector(
                PS3300_PATCH_MODEL_NAMES
                    .iter()
                    .map(|name| Expr::String((*name).to_owned()))
                    .collect(),
            ),
        )
        .with_setting(
            setting_key("section-count"),
            number_usize(PS3300_SECTION_COUNT),
        )
        .with_setting(setting_key("key-count"), number_usize(PS3300_KEY_COUNT))
        .with_setting(
            setting_key("total-key-cells"),
            number_usize(PS3300_TOTAL_KEY_CELLS),
        )
        .with_module(
            PatchModule::new(Symbol::new("in"), Symbol::new("in"))
                .with_output(PatchJack::event("key-out", true)),
        )
        .with_module(
            PatchModule::new(Symbol::new("out"), Symbol::new("out"))
                .with_input(PatchJack::audio("in", true)),
        )
        .with_module(module(
            "keyboard",
            "ps3-keyboard-controller",
            vec![PatchJack::event("key-in", false)],
            vec![
                PatchJack::cv("pitch-cv-out", true),
                PatchJack::gate("gate-out", true),
            ],
            vec![(
                setting_key("role"),
                Expr::Symbol(Ps3300ModuleRole::Keyboard.symbol()),
            )],
        ))
        .with_module(module(
            "pin-matrix",
            "ps3-pin-matrix",
            vec![
                PatchJack::cv("keyboard-pitch-cv", true),
                PatchJack::gate("keyboard-gate", true),
                PatchJack::audio("section-a-audio", false),
                PatchJack::audio("section-b-audio", false),
                PatchJack::audio("section-c-audio", false),
            ],
            vec![
                PatchJack::cv("section-a-pitch-cv", true),
                PatchJack::cv("section-b-pitch-cv", true),
                PatchJack::cv("section-c-pitch-cv", true),
                PatchJack::gate("section-a-gate", true),
                PatchJack::gate("section-b-gate", true),
                PatchJack::gate("section-c-gate", true),
                PatchJack::audio("resonator-audio-in", false),
            ],
            vec![(setting_key("route-count"), number_usize(routes.len()))],
        ));

    for section in ps3300_sections() {
        patch = patch.with_module(section_module(section));
    }

    patch
        .with_module(module(
            "resonator",
            "ps3-resonator-bank",
            vec![
                PatchJack::audio("audio-in-a", true),
                PatchJack::audio("audio-in-b", true),
                PatchJack::audio("audio-in-c", true),
            ],
            vec![PatchJack::audio("audio-out", true)],
            vec![(
                setting_key("resonator-settings"),
                ps3300_resonator_settings().to_expr(),
            )],
        ))
        .with_module(module(
            "output-mixer",
            "ps3-output-mixer",
            vec![PatchJack::audio("audio-in", true)],
            vec![PatchJack::audio("audio-out", true)],
            vec![(
                setting_key("section-count"),
                number_usize(PS3300_SECTION_COUNT),
            )],
        ))
        .with_cord(PatchCord::new(
            PatchEndpoint::new("in", "key-out"),
            endpoint("keyboard", "key-in"),
        ))
        .with_cord(cord(
            "keyboard",
            "pitch-cv-out",
            "pin-matrix",
            "keyboard-pitch-cv",
        ))
        .with_cord(cord("keyboard", "gate-out", "pin-matrix", "keyboard-gate"))
        .with_cord(cord(
            "pin-matrix",
            "section-a-pitch-cv",
            "section-a",
            "pitch-cv-in",
        ))
        .with_cord(cord(
            "pin-matrix",
            "section-b-pitch-cv",
            "section-b",
            "pitch-cv-in",
        ))
        .with_cord(cord(
            "pin-matrix",
            "section-c-pitch-cv",
            "section-c",
            "pitch-cv-in",
        ))
        .with_cord(cord("pin-matrix", "section-a-gate", "section-a", "gate-in"))
        .with_cord(cord("pin-matrix", "section-b-gate", "section-b", "gate-in"))
        .with_cord(cord("pin-matrix", "section-c-gate", "section-c", "gate-in"))
        .with_cord(cord("section-a", "audio-out", "resonator", "audio-in-a"))
        .with_cord(cord("section-b", "audio-out", "resonator", "audio-in-b"))
        .with_cord(cord("section-c", "audio-out", "resonator", "audio-in-c"))
        .with_cord(cord("resonator", "audio-out", "output-mixer", "audio-in"))
        .with_cord(PatchCord::new(
            endpoint("output-mixer", "audio-out"),
            PatchEndpoint::new("out", "in"),
        ))
}

/// Builds the PS-3300 per-key-cell patch: one voice-cell module per key per section.
pub fn ps3300_per_key_cell_patch() -> InstrumentPatch {
    let mut patch = InstrumentPatch::new(ps3300_per_key_cell_patch_id()).with_setting(
        setting_key("total-key-cells"),
        number_usize(PS3300_TOTAL_KEY_CELLS),
    );
    let assignment = ps3300_keyboard_assignment();
    for (section_index, array) in ps3300_section_polyphonic_arrays().into_iter().enumerate() {
        let section = ps3300_sections()[section_index];
        for voice_index in 0..array.voice_count() {
            let midi_key = usize::from(assignment.first_midi_key) + voice_index;
            patch = patch.with_module(
                PatchModule::new(
                    Symbol::qualified(
                        "audio-synth/ps3300-cell",
                        format!("{}-key-{voice_index:02}", section.as_str()),
                    ),
                    ps3300_module_id("ps3-per-key-cell"),
                )
                .with_input(PatchJack::cv("pitch-cv-in", true))
                .with_input(PatchJack::gate("gate-in", true))
                .with_output(PatchJack::audio("audio-out", true))
                .with_setting(setting_key("section"), Expr::Symbol(section.symbol()))
                .with_setting(setting_key("poly-array"), Expr::Symbol(array.id().clone()))
                .with_setting(setting_key("voice-index"), number_usize(voice_index))
                .with_setting(setting_key("midi-key"), number_usize(midi_key)),
            );
        }
    }
    patch
}

fn ps3300_raw_view(routes: &[Ps3300PinMatrixRoute]) -> PatchRawView {
    PatchRawView::new(Symbol::qualified("audio-synth/raw", "korg-ps-3300"))
        .with_field(
            setting_key("keyboard-assignment"),
            ps3300_keyboard_assignment().to_expr(),
        )
        .with_field(
            setting_key("section-count"),
            number_usize(PS3300_SECTION_COUNT),
        )
        .with_field(setting_key("key-count"), number_usize(PS3300_KEY_COUNT))
        .with_field(
            setting_key("resonator"),
            ps3300_resonator_settings().to_expr(),
        )
        .with_field(
            setting_key("pin-matrix-routes"),
            Expr::Vector(routes.iter().map(Ps3300PinMatrixRoute::to_expr).collect()),
        )
}

fn section_module(section: Ps3300Section) -> PatchModule {
    module(
        section.as_str(),
        "ps3-section-generator",
        vec![
            PatchJack::cv("pitch-cv-in", true),
            PatchJack::gate("gate-in", true),
        ],
        vec![PatchJack::audio("audio-out", true)],
        vec![
            (setting_key("section"), Expr::Symbol(section.symbol())),
            (setting_key("key-count"), number_usize(PS3300_KEY_COUNT)),
            (
                setting_key("poly-array"),
                Expr::Symbol(ps3300_polyphonic_array(section).id().clone()),
            ),
        ],
    )
}

fn module(
    instance_name: &'static str,
    module_id: &'static str,
    inputs: Vec<PatchJack>,
    outputs: Vec<PatchJack>,
    settings: Vec<(Symbol, Expr)>,
) -> PatchModule {
    let mut module = PatchModule::new(instance_id(instance_name), ps3300_module_id(module_id));
    for input in inputs {
        module = module.with_input(input);
    }
    for output in outputs {
        module = module.with_output(output);
    }
    for (key, value) in settings {
        module = module.with_setting(key, value);
    }
    module
}

fn instance_id(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/ps3300", name)
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
        module: instance_id(module),
        jack: Symbol::new(jack),
    }
}
