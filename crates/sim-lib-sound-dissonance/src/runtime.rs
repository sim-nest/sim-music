use sim_kernel::{
    AbiVersion, Cx, Export, ExportKind, ExportRecord, ExportState, Lib, LibManifest, LibTarget,
    Linker, Result, RuntimeId, Symbol, Version,
};

use crate::DissonanceModelDescriptor;

const SOUND_DISSONANCE_LIB_ID: &str = "sound-dissonance";
const MODEL_EXPORT_KIND: &str = "DissonanceModel";
const REGISTRY_SYMBOL_NAME: &str = "DissonanceRegistry";

/// Host-registered lib exporting the built-in dissonance-model cards and the
/// dissonance registry.
pub struct SoundDissonanceLib;

impl Lib for SoundDissonanceLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(SOUND_DISSONANCE_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: model_symbols()
                .into_iter()
                .chain(std::iter::once(registry_symbol()))
                .map(|symbol| Export::Value { symbol })
                .collect(),
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for symbol in model_symbols() {
            linker.value(symbol.clone(), model_value(cx, symbol.clone())?)?;
        }
        linker.value(registry_symbol(), registry_value(cx)?)?;
        Ok(())
    }
}

/// Installs the sound-dissonance lib into `cx`, registering the built-in
/// dissonance-model cards and registry export records (idempotent).
pub fn install_sound_dissonance_lib(cx: &mut Cx) -> Result<()> {
    let lib = Symbol::new(SOUND_DISSONANCE_LIB_ID);
    if !sim_lib_core::install_once(cx, &SoundDissonanceLib)? {
        return Ok(());
    }
    for symbol in model_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(MODEL_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    Ok(())
}

fn model_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("sound", "PlompLevelt"),
        Symbol::qualified("sound", "Sethares"),
        Symbol::qualified("sound", "HelmholtzBeating"),
        Symbol::qualified("sound", "HarmonicEntropy"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("sound", REGISTRY_SYMBOL_NAME)
}

fn registry_value(cx: &mut sim_kernel::LoadCx) -> Result<sim_kernel::Value> {
    let models = cx.factory().list(
        model_symbols()
            .into_iter()
            .map(|symbol| cx.factory().symbol(symbol))
            .collect::<Result<Vec<_>>>()?,
    )?;
    cx.factory().table(vec![
        (
            Symbol::new("symbol"),
            cx.factory().symbol(registry_symbol())?,
        ),
        (
            Symbol::new("layer"),
            cx.factory().string("sound".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("sound", "DissonanceModel"))?,
        ),
        (Symbol::new("dependencies"), cx.factory().list(Vec::new())?),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("models"), models),
    ])
}

fn model_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let descriptor = match &*symbol.name {
        "PlompLevelt" => format!("{:?}", DissonanceModelDescriptor::PlompLevelt),
        "Sethares" => format!("{:?}", DissonanceModelDescriptor::Sethares),
        "HelmholtzBeating" => format!("{:?}", DissonanceModelDescriptor::HelmholtzBeating),
        "HarmonicEntropy" => format!(
            "{:?}",
            DissonanceModelDescriptor::HarmonicEntropy { spread: 18.0 }
        ),
        _ => String::new(),
    };
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol)?),
        (
            Symbol::new("layer"),
            cx.factory().string("sound".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("sound", "DissonanceModel"))?,
        ),
        (Symbol::new("dependencies"), cx.factory().list(Vec::new())?),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("descriptor"), cx.factory().string(descriptor)?),
    ])
}
