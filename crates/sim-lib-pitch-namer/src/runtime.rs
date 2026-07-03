use sim_kernel::{
    Cx, ExportKind, ExportRecord, ExportState, Lib, LibManifest, Linker, LoadCx, Result, RuntimeId,
    Symbol,
};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const PITCH_NAMER_LIB_ID: &str = "pitch-namer";
const CLUSTER_NAMER_EXPORT_KIND: &str = "ClusterNamer";
const REGISTRY_SYMBOL_NAME: &str = "NamerRegistry";

/// Host-registered lib exporting the built-in cluster-namer cards and registry,
/// built on the shared [`SurfacePackLib`] substrate.
pub struct PitchNamerLib;

impl Lib for PitchNamerLib {
    fn manifest(&self) -> LibManifest {
        pitch_namer_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        pitch_namer_pack().load(cx, linker)
    }
}

/// Installs the [`PitchNamerLib`] into `cx`, registering each namer's export
/// record; installing more than once is a no-op.
pub fn install_pitch_namer_lib(cx: &mut Cx) -> Result<()> {
    if !install_once(cx, &PitchNamerLib)? {
        return Ok(());
    }
    let lib = Symbol::new(PITCH_NAMER_LIB_ID);
    for symbol in namer_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(CLUSTER_NAMER_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    Ok(())
}

fn namer_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("pitch", "ForteNamer"),
        Symbol::qualified("pitch", "FunctionalRomanNamer"),
        Symbol::qualified("pitch", "SetTheoryNamer"),
        Symbol::qualified("pitch", "RiemannNamer"),
        Symbol::qualified("pitch", "JazzNamer"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("pitch", REGISTRY_SYMBOL_NAME)
}

fn registry_value_spec() -> SurfaceValueSpec {
    SurfaceValueSpec {
        symbol: registry_symbol(),
        fields: vec![
            (
                Symbol::new("symbol"),
                SurfaceField::Symbol(registry_symbol()),
            ),
            (Symbol::new("layer"), SurfaceField::Str("pitch".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("shape"),
                SurfaceField::Symbol(Symbol::qualified("pitch", "ClusterNamer")),
            ),
            (Symbol::new("dependencies"), SurfaceField::Symbols(vec![])),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (
                Symbol::new("namers"),
                SurfaceField::Symbols(namer_symbols()),
            ),
        ],
    }
}

fn namer_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let school = symbol.name.to_string();
    SurfaceValueSpec {
        symbol: symbol.clone(),
        fields: vec![
            (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
            (Symbol::new("layer"), SurfaceField::Str("pitch".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("shape"),
                SurfaceField::Symbol(Symbol::qualified("pitch", "ClusterNamer")),
            ),
            (
                Symbol::new("dependencies"),
                SurfaceField::Strs(vec![
                    "pitch-core".to_owned(),
                    "pitch-set".to_owned(),
                    "pitch-chord".to_owned(),
                ]),
            ),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (Symbol::new("school"), SurfaceField::Str(school)),
        ],
    }
}

fn pitch_namer_pack() -> SurfacePackLib {
    let mut values: Vec<SurfaceValueSpec> =
        namer_symbols().into_iter().map(namer_value_spec).collect();
    values.push(registry_value_spec());
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(PITCH_NAMER_LIB_ID),
            values,
        },
    }
}
