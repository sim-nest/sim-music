use sim_kernel::{
    AbiVersion, Cx, Export, ExportKind, ExportRecord, ExportState, Lib, LibManifest, LibTarget,
    Linker, Result, RuntimeId, Symbol, Version,
};

const PITCH_DISSONANCE_LIB_ID: &str = "pitch-dissonance";
const MODEL_EXPORT_KIND: &str = "PitchDissonanceModel";
const REGISTRY_SYMBOL_NAME: &str = "PitchDissonanceRegistry";

/// The SIM runtime library that exports the built-in dissonance models and their
/// registry.
pub struct PitchDissonanceLib;

impl Lib for PitchDissonanceLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(PITCH_DISSONANCE_LIB_ID),
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

/// Installs the [`PitchDissonanceLib`] into `cx`, registering each model's export
/// record; installing more than once is a no-op.
pub fn install_pitch_dissonance_lib(cx: &mut Cx) -> Result<()> {
    let lib = Symbol::new(PITCH_DISSONANCE_LIB_ID);
    if !sim_lib_core::install_once(cx, &PitchDissonanceLib)? {
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
        Symbol::qualified("pitch", "IntervalVectorModel"),
        Symbol::qualified("pitch", "ForteComplexity"),
        Symbol::qualified("pitch", "TonalFunctionDissonance"),
        Symbol::qualified("pitch", "TritoneDensity"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("pitch", REGISTRY_SYMBOL_NAME)
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
            cx.factory().string("pitch".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("pitch", "PitchDissonanceModel"))?,
        ),
        (Symbol::new("dependencies"), cx.factory().list(Vec::new())?),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("models"), models),
    ])
}

fn model_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let name = symbol.name.to_string();
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol)?),
        (
            Symbol::new("layer"),
            cx.factory().string("pitch".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("pitch", "PitchDissonanceModel"))?,
        ),
        (
            Symbol::new("dependencies"),
            cx.factory().list(vec![
                cx.factory().string("pitch-set".to_owned())?,
                cx.factory().string("pitch-scale".to_owned())?,
            ])?,
        ),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("model"), cx.factory().string(name)?),
    ])
}
