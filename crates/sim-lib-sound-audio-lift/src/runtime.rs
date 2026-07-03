use sim_kernel::{
    AbiVersion, Cx, Export, ExportKind, ExportRecord, ExportState, Lib, LibManifest, LibTarget,
    Linker, Result, RuntimeId, Symbol, Version,
};

const SOUND_AUDIO_LIFT_LIB_ID: &str = "sound-audio-lift";
const AUDIO_LIFTER_EXPORT_KIND: &str = "AudioLifter";
const REGISTRY_SYMBOL_NAME: &str = "AudioLifterRegistry";

/// Host-registered lib exporting the built-in audio-lifter cards and registry.
pub struct SoundAudioLiftLib;

impl Lib for SoundAudioLiftLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(SOUND_AUDIO_LIFT_LIB_ID),
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

/// Installs the sound-audio-lift lib into `cx`, registering the built-in
/// audio-lifter cards and registry export records (idempotent).
pub fn install_sound_audio_lift_lib(cx: &mut Cx) -> Result<()> {
    let lib = Symbol::new(SOUND_AUDIO_LIFT_LIB_ID);
    if !sim_lib_core::install_once(cx, &SoundAudioLiftLib)? {
        return Ok(());
    }
    for symbol in lifter_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(AUDIO_LIFTER_EXPORT_KIND),
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
        Symbol::qualified("sound", "FftPeakLifter"),
        Symbol::qualified("sound", "HarmonicCombLifter"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("sound", REGISTRY_SYMBOL_NAME)
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
            cx.factory().string("sound".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("sound", "AudioLifter"))?,
        ),
        (Symbol::new("dependencies"), cx.factory().list(Vec::new())?),
        (Symbol::new("lossless"), cx.factory().bool(false)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("lifters"), lifters),
    ])
}

fn lifter_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let dependencies = cx.factory().list(vec![
        cx.factory().string("sound-spectrum".to_owned())?,
        cx.factory().string("sound-tuning".to_owned())?,
        cx.factory().string("pitch-core".to_owned())?,
    ])?;
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol.clone())?),
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
                .symbol(Symbol::qualified("sound", "AudioLifter"))?,
        ),
        (Symbol::new("dependencies"), dependencies),
        (Symbol::new("lossless"), cx.factory().bool(false)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (
            Symbol::new("lifter"),
            cx.factory().string(symbol.name.to_string())?,
        ),
    ])
}
