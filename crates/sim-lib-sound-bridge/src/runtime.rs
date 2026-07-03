use sim_kernel::{
    AbiVersion, Cx, Export, ExportKind, ExportRecord, ExportState, Lib, LibManifest, LibTarget,
    Linker, Result, RuntimeId, Symbol, Version,
};

const SOUND_BRIDGE_LIB_ID: &str = "sound-bridge";
const BRIDGE_EXPORT_KIND: &str = "Bridge";
const REGISTRY_SYMBOL_NAME: &str = "BridgeRegistry";

/// Host-registered lib exporting the MIDI-to-sound bridge cards and registry.
pub struct SoundBridgeLib;

impl Lib for SoundBridgeLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(SOUND_BRIDGE_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: bridge_symbols()
                .into_iter()
                .chain(std::iter::once(registry_symbol()))
                .map(|symbol| Export::Value { symbol })
                .collect(),
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for symbol in bridge_symbols() {
            linker.value(symbol.clone(), bridge_value(cx, symbol.clone())?)?;
        }
        linker.value(registry_symbol(), registry_value(cx)?)?;
        Ok(())
    }
}

/// Installs the sound-bridge lib into `cx`, registering the bridge cards and
/// registry export records (idempotent).
pub fn install_sound_bridge_lib(cx: &mut Cx) -> Result<()> {
    let lib = Symbol::new(SOUND_BRIDGE_LIB_ID);
    if !sim_lib_core::install_once(cx, &SoundBridgeLib)? {
        return Ok(());
    }
    for symbol in bridge_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(BRIDGE_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    Ok(())
}

fn bridge_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("sound", "MidiToSoundBridge"),
        Symbol::qualified("sound", "VoicePool"),
        Symbol::qualified("sound", "TimbreBank"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("sound", REGISTRY_SYMBOL_NAME)
}

fn registry_value(cx: &mut sim_kernel::LoadCx) -> Result<sim_kernel::Value> {
    let bridges = cx.factory().list(
        bridge_symbols()
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
            cx.factory().string("bridge".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("sound", "MidiToSoundBridge"))?,
        ),
        (Symbol::new("dependencies"), cx.factory().list(Vec::new())?),
        (Symbol::new("lossless"), cx.factory().bool(false)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("bridges"), bridges),
    ])
}

fn bridge_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let dependencies = match &*symbol.name {
        "MidiToSoundBridge" => vec![
            cx.factory().string("midi-core".to_owned())?,
            cx.factory().string("sound-timbre".to_owned())?,
            cx.factory().string("sound-tuning".to_owned())?,
        ],
        "VoicePool" => vec![cx.factory().string("sound-core".to_owned())?],
        "TimbreBank" => vec![cx.factory().string("sound-timbre".to_owned())?],
        _ => Vec::new(),
    };
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol)?),
        (
            Symbol::new("layer"),
            cx.factory().string("sound".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("bridge".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("sound", "MidiToSoundBridge"))?,
        ),
        (
            Symbol::new("dependencies"),
            cx.factory().list(dependencies)?,
        ),
        (Symbol::new("lossless"), cx.factory().bool(false)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
    ])
}
