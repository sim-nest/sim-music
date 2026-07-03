use sim_kernel::{
    Cx, ExportKind, ExportRecord, ExportState, Lib, LibManifest, Linker, LoadCx, Result, RuntimeId,
    Symbol,
};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{
    bell_inharmonic, fm_pair, karplus_strong, organ_pipe, pure_sine, sawtooth, square, triangle,
};

const SOUND_TIMBRE_LIB_ID: &str = "sound-timbre";
const EXPORT_KIND_NAME: &str = "Timbre";
const REGISTRY_SYMBOL_NAME: &str = "TimbreRegistry";

/// Host-registered lib exporting the built-in timbre cards and registry, built
/// on the shared [`SurfacePackLib`] substrate.
pub struct SoundTimbreLib;

impl Lib for SoundTimbreLib {
    fn manifest(&self) -> LibManifest {
        sound_timbre_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        sound_timbre_pack().load(cx, linker)
    }
}

/// Installs the sound-timbre lib into `cx`, registering the built-in timbre
/// cards and the timbre registry export records (idempotent).
pub fn install_sound_timbre_lib(cx: &mut Cx) -> Result<()> {
    if !install_once(cx, &SoundTimbreLib)? {
        return Ok(());
    }
    let lib = Symbol::new(SOUND_TIMBRE_LIB_ID);
    for symbol in timbre_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(EXPORT_KIND_NAME),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    Ok(())
}

fn timbre_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("sound", "PureSine"),
        Symbol::qualified("sound", "Sawtooth"),
        Symbol::qualified("sound", "Square"),
        Symbol::qualified("sound", "Triangle"),
        Symbol::qualified("sound", "OrganPipe"),
        Symbol::qualified("sound", "KarplusStrong"),
        Symbol::qualified("sound", "FmPair"),
        Symbol::qualified("sound", "BellInharmonic"),
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
                SurfaceField::Symbol(Symbol::qualified("sound", "Timbre")),
            ),
            (Symbol::new("dependencies"), SurfaceField::Symbols(vec![])),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (
                Symbol::new("timbres"),
                SurfaceField::Symbols(timbre_symbols()),
            ),
        ],
    }
}

fn timbre_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let timbre = match symbol.name.as_ref() {
        "PureSine" => pure_sine(),
        "Sawtooth" => sawtooth(8),
        "Square" => square(8),
        "Triangle" => triangle(8),
        "OrganPipe" => organ_pipe(&[1.0, 2.0, 4.0]),
        "KarplusStrong" => karplus_strong(0.82),
        "FmPair" => fm_pair(2.0, 3.0),
        "BellInharmonic" => bell_inharmonic(&[1.0, 2.76, 5.41]),
        _ => pure_sine(),
    };
    SurfaceValueSpec {
        symbol: symbol.clone(),
        fields: vec![
            (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
            (Symbol::new("layer"), SurfaceField::Str("sound".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("shape"),
                SurfaceField::Symbol(Symbol::qualified("sound", "Timbre")),
            ),
            (
                Symbol::new("dependencies"),
                SurfaceField::Strs(vec!["sound-core".to_owned(), "sound-spectrum".to_owned()]),
            ),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (
                Symbol::new("timbre-name"),
                SurfaceField::Str(encode_timbre_name(&timbre.name)),
            ),
            (
                Symbol::new("category"),
                SurfaceField::Str(timbre.metadata.category),
            ),
        ],
    }
}

fn sound_timbre_pack() -> SurfacePackLib {
    let mut values: Vec<SurfaceValueSpec> = timbre_symbols()
        .into_iter()
        .map(timbre_value_spec)
        .collect();
    values.push(registry_value_spec());
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(SOUND_TIMBRE_LIB_ID),
            values,
        },
    }
}

fn encode_timbre_name(name: &str) -> String {
    name.to_owned()
}
