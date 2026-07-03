use sim_kernel::{
    AbiVersion, Cx, Export, ExportKind, ExportRecord, ExportState, Lib, LibManifest, LibTarget,
    Linker, Result, RuntimeId, Symbol, Version,
};

const MIDI_LIVE_LIB_ID: &str = "midi-live";
const SOURCE_EXPORT_KIND: &str = "MidiSourceFactory";
const SINK_EXPORT_KIND: &str = "MidiSinkFactory";
const TRACKED_SOURCE_EXPORT_KIND: &str = "TrackedMidiSourceFactory";
const REGISTRY_SYMBOL_NAME: &str = "MidiLiveRegistry";

/// Host-registered lib exposing the ring-buffer source/sink cards and their
/// registry to a running runtime.
pub struct MidiLiveLib;

impl Lib for MidiLiveLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::new(MIDI_LIVE_LIB_ID),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: live_symbols()
                .into_iter()
                .chain(std::iter::once(registry_symbol()))
                .map(|symbol| Export::Value { symbol })
                .collect(),
        }
    }

    fn load(&self, cx: &mut sim_kernel::LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        for symbol in live_symbols() {
            linker.value(symbol.clone(), live_value(cx, symbol.clone())?)?;
        }
        linker.value(registry_symbol(), registry_value(cx)?)?;
        Ok(())
    }
}

/// Installs [`MidiLiveLib`] into `cx` once and registers the ring-buffer
/// source, sink, and tracked-source export records.
pub fn install_midi_live_lib(cx: &mut Cx) -> Result<()> {
    let lib = Symbol::new(MIDI_LIVE_LIB_ID);
    if !sim_lib_core::install_once(cx, &MidiLiveLib)? {
        return Ok(());
    }
    for symbol in source_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(SOURCE_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    for symbol in sink_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(SINK_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    for symbol in tracked_source_symbols() {
        cx.registry_mut().append_export_record(
            &lib,
            ExportRecord {
                kind: ExportKind::named(TRACKED_SOURCE_EXPORT_KIND),
                symbol,
                state: ExportState::Resolved {
                    id: RuntimeId::Value,
                },
            },
        )?;
    }
    Ok(())
}

fn source_symbols() -> Vec<Symbol> {
    vec![Symbol::qualified("midi", "RingMidiBuffer")]
}

fn sink_symbols() -> Vec<Symbol> {
    vec![Symbol::qualified("midi", "RingMidiBuffer")]
}

fn tracked_source_symbols() -> Vec<Symbol> {
    vec![Symbol::qualified("midi", "RingTrackedMidiBuffer")]
}

fn live_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("midi", "RingMidiBuffer"),
        Symbol::qualified("midi", "RingTrackedMidiBuffer"),
    ]
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("midi", REGISTRY_SYMBOL_NAME)
}

fn registry_value(cx: &mut sim_kernel::LoadCx) -> Result<sim_kernel::Value> {
    let buffers = cx.factory().list(
        live_symbols()
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
            cx.factory().string("midi".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (
            Symbol::new("shape"),
            cx.factory()
                .symbol(Symbol::qualified("midi", "TrackedMidiSourceFactory"))?,
        ),
        (Symbol::new("dependencies"), cx.factory().list(Vec::new())?),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("buffers"), buffers),
    ])
}

fn live_value(cx: &mut sim_kernel::LoadCx, symbol: Symbol) -> Result<sim_kernel::Value> {
    let (shape, role) = match symbol.name.as_ref() {
        "RingMidiBuffer" => (
            Symbol::qualified("midi", "MidiSinkFactory"),
            "ring buffer source/sink",
        ),
        "RingTrackedMidiBuffer" => (
            Symbol::qualified("midi", "TrackedMidiSourceFactory"),
            "ring buffer tracked source",
        ),
        _ => (Symbol::qualified("midi", "MidiSourceFactory"), "unknown"),
    };
    cx.factory().table(vec![
        (Symbol::new("symbol"), cx.factory().symbol(symbol)?),
        (
            Symbol::new("layer"),
            cx.factory().string("midi".to_owned())?,
        ),
        (
            Symbol::new("kind"),
            cx.factory().string("plugin".to_owned())?,
        ),
        (Symbol::new("shape"), cx.factory().symbol(shape)?),
        (
            Symbol::new("dependencies"),
            cx.factory().list(vec![
                cx.factory().string("midi-core".to_owned())?,
                cx.factory().string("midi-live".to_owned())?,
            ])?,
        ),
        (Symbol::new("lossless"), cx.factory().bool(true)?),
        (Symbol::new("capabilities"), cx.factory().list(Vec::new())?),
        (Symbol::new("role"), cx.factory().string(role.to_owned())?),
    ])
}
