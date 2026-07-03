use sim_kernel::{
    AbiVersion, Cx, Export, ExportKind, ExportRecord, ExportState, Lib, LibManifest, LibTarget,
    Linker, Result, RuntimeId, Symbol, Version,
};

const MUSIC_LIFT_LIB_ID: &str = "music-lift";
const MIDI_LIFTER_EXPORT_KIND: &str = "MidiLifter";
const REGISTRY_SYMBOL_NAME: &str = "MidiLifterRegistry";

/// Loadable library that registers the MIDI lifters as host-side runtime values.
///
/// It exports one value per lifter symbol plus a `MidiLifterRegistry` table that
/// enumerates them with their layer, shape, and capability metadata.
pub struct MusicLiftLib;

impl Lib for MusicLiftLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(MUSIC_LIFT_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: lifter_symbols()
                .into_iter()
                .chain(std::iter::once(registry_symbol()))
                .map(|symbol| Export::Value { symbol })
                .collect(),
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for symbol in lifter_symbols() {
            linker.value(symbol.clone(), lifter_value(cx, symbol.clone())?)?;
        }
        linker.value(registry_symbol(), registry_value(cx)?)?;
        Ok(())
    }
}

/// Loads [`MusicLiftLib`] into `cx` once, recording lifter export records.
pub fn install_music_lift_lib(cx: &mut Cx) -> Result<()> {
    let lib = Symbol::new(MUSIC_LIFT_LIB_ID);
    if !sim_lib_core::install_once(cx, &MusicLiftLib)? {
        return Ok(());
    }
    for symbol in lifter_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(MIDI_LIFTER_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    Ok(())
}

fn lifter_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("music", "MidiToPianoRoll"),
        Symbol::qualified("music", "MidiToDiffRoll"),
        Symbol::qualified("music", "MidiToProgression"),
        Symbol::qualified("music", "MidiToCounterpoint"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("music", REGISTRY_SYMBOL_NAME)
}

fn registry_value(cx: &mut sim_kernel::LoadCx) -> Result<sim_kernel::Value> {
    let lifters = cx.factory().list(
        lifter_symbols()
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
            cx.factory().string("music".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("music", "MidiLifter"))?,
        ),
        (Symbol::new("dependencies"), cx.factory().list(Vec::new())?),
        (Symbol::new("lossless"), cx.factory().bool(false)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("lifters"), lifters),
    ])
}

fn lifter_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let name = symbol.name.to_string();
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol)?),
        (
            Symbol::new("layer"),
            cx.factory().string("music".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("music", "MidiLifter"))?,
        ),
        (
            Symbol::new("dependencies"),
            cx.factory().list(vec![
                cx.factory().string("midi-smf".to_owned())?,
                cx.factory().string("music-core".to_owned())?,
                cx.factory().string("music-analysis".to_owned())?,
            ])?,
        ),
        (Symbol::new("lossless"), cx.factory().bool(false)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("lifter"), cx.factory().string(name)?),
    ])
}
