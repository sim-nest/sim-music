use sim_kernel::{
    Cx, ExportKind, ExportRecord, ExportState, Lib, LibManifest, Linker, LoadCx, Result, RuntimeId,
    Symbol,
};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{TuningDescriptor, default_just_intonation};

const SOUND_TUNING_LIB_ID: &str = "sound-tuning";
const TUNING_EXPORT_KIND: &str = "Tuning";
const REGISTRY_SYMBOL_NAME: &str = "TuningRegistry";

/// Host-registered lib exporting the built-in tuning cards and registry, built
/// on the shared [`SurfacePackLib`] substrate.
pub struct SoundTuningLib;

impl Lib for SoundTuningLib {
    fn manifest(&self) -> LibManifest {
        sound_tuning_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        sound_tuning_pack().load(cx, linker)
    }
}

/// Installs the sound-tuning lib into `cx`, registering the built-in tuning
/// cards and the tuning registry export records (idempotent).
pub fn install_sound_tuning_lib(cx: &mut Cx) -> Result<()> {
    if !install_once(cx, &SoundTuningLib)? {
        return Ok(());
    }
    let lib = Symbol::new(SOUND_TUNING_LIB_ID);
    for symbol in tuning_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(TUNING_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    Ok(())
}

fn tuning_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("sound", "EqualTemperament"),
        Symbol::qualified("sound", "JustIntonation"),
        Symbol::qualified("sound", "PythagoreanTuning"),
        Symbol::qualified("sound", "MeantoneQuarterComma"),
        Symbol::qualified("sound", "WerckmeisterIII"),
        Symbol::qualified("sound", "YoungTemperament"),
        Symbol::qualified("sound", "ScalaScl"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("sound", REGISTRY_SYMBOL_NAME)
}

fn registry_value_spec() -> SurfaceValueSpec {
    SurfaceValueSpec {
        symbol: registry_symbol(),
        fields: vec![
            (
                Symbol::new("symbol"),
                SurfaceField::Symbol(registry_symbol()),
            ),
            (Symbol::new("layer"), SurfaceField::Str("sound".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("shape"),
                SurfaceField::Symbol(Symbol::qualified("sound", "Tuning")),
            ),
            (Symbol::new("dependencies"), SurfaceField::Symbols(vec![])),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (
                Symbol::new("tunings"),
                SurfaceField::Symbols(tuning_symbols()),
            ),
        ],
    }
}

fn tuning_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let descriptor = match &*symbol.name {
        "EqualTemperament" => format!(
            "{:?}",
            TuningDescriptor::EqualTemperament {
                divisions: 12,
                reference_midi: 69,
                reference_hz: 440.0,
            }
        ),
        "JustIntonation" => {
            let just = default_just_intonation();
            format!(
                "{:?}",
                TuningDescriptor::JustIntonation {
                    root: just.root.0,
                    ratios: just.ratios,
                    reference_midi: 69,
                    reference_hz: 440.0,
                }
            )
        }
        "PythagoreanTuning" => format!(
            "{:?}",
            TuningDescriptor::PythagoreanTuning {
                reference_midi: 69,
                reference_hz: 440.0,
            }
        ),
        "MeantoneQuarterComma" => format!(
            "{:?}",
            TuningDescriptor::MeantoneQuarterComma {
                reference_midi: 69,
                reference_hz: 440.0,
            }
        ),
        "WerckmeisterIII" => format!(
            "{:?}",
            TuningDescriptor::WerckmeisterIII {
                reference_midi: 69,
                reference_hz: 440.0,
            }
        ),
        "YoungTemperament" => format!(
            "{:?}",
            TuningDescriptor::YoungTemperament {
                reference_midi: 69,
                reference_hz: 440.0,
            }
        ),
        "ScalaScl" => format!(
            "{:?}",
            TuningDescriptor::ScalaScl {
                cents: vec![
                    0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0,
                    1100.0
                ],
                reference_midi: 69,
                reference_hz: 440.0,
            }
        ),
        _ => String::new(),
    };
    SurfaceValueSpec {
        symbol: symbol.clone(),
        fields: vec![
            (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
            (Symbol::new("layer"), SurfaceField::Str("sound".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("shape"),
                SurfaceField::Symbol(Symbol::qualified("sound", "Tuning")),
            ),
            (Symbol::new("dependencies"), SurfaceField::Symbols(vec![])),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (Symbol::new("descriptor"), SurfaceField::Str(descriptor)),
        ],
    }
}

fn sound_tuning_pack() -> SurfacePackLib {
    let mut values: Vec<SurfaceValueSpec> = tuning_symbols()
        .into_iter()
        .map(tuning_value_spec)
        .collect();
    values.push(registry_value_spec());
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(SOUND_TUNING_LIB_ID),
            values,
        },
    }
}
