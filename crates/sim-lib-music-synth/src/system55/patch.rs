use sim_kernel::{Expr, Symbol};

use super::{
    SYSTEM55_MODULE_DESCRIPTORS, System55ModuleRole, m55_module_id, system55_s_trigger_convention,
    system55_scaffold_patch_id,
};
use crate::{GateConvention, InstrumentPatch, PatchCord, PatchEndpoint, PatchJack, PatchModule};

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

fn module_instance_id(name: &'static str) -> Symbol {
    Symbol::qualified("audio-synth/system55", name)
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
