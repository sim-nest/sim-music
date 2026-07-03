use sim_kernel::{
    Cx, ExportKind, ExportRecord, ExportState, Lib, LibManifest, Linker, LoadCx, Result, RuntimeId,
    Symbol,
};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const MIDI_IO_LIB_ID: &str = "midi-io";
const SOURCE_EXPORT_KIND: &str = "MidiSourceFactory";
const SINK_EXPORT_KIND: &str = "MidiSinkFactory";
const TRACKED_SOURCE_EXPORT_KIND: &str = "TrackedMidiSourceFactory";
const REGISTRY_SYMBOL_NAME: &str = "MidiIoRegistry";

/// Host-registered lib exporting the in-memory MIDI I/O cards and registry,
/// built on the shared [`SurfacePackLib`] substrate.
pub struct MidiIoLib;

impl Lib for MidiIoLib {
    fn manifest(&self) -> LibManifest {
        midi_io_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        midi_io_pack().load(cx, linker)
    }
}

/// Installs [`MidiIoLib`] into `cx` once and registers the in-memory source,
/// sink, and tracked-source export records.
///
/// Returns `Ok(())` immediately if the lib is already installed.
pub fn install_midi_io_lib(cx: &mut Cx) -> Result<()> {
    if !install_once(cx, &MidiIoLib)? {
        return Ok(());
    }
    let lib = Symbol::new(MIDI_IO_LIB_ID);
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
    vec![Symbol::qualified("midi", "MemoryMidiSource")]
}

fn sink_symbols() -> Vec<Symbol> {
    vec![Symbol::qualified("midi", "MemoryMidiSink")]
}

fn tracked_source_symbols() -> Vec<Symbol> {
    vec![Symbol::qualified("midi", "MemoryTrackedMidiSource")]
}

fn io_symbols() -> Vec<Symbol> {
    let mut symbols = source_symbols();
    symbols.extend(sink_symbols());
    symbols.extend(tracked_source_symbols());
    symbols
}

fn registry_symbol() -> Symbol {
    Symbol::qualified("midi", REGISTRY_SYMBOL_NAME)
}

fn io_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let (shape, mode) = match symbol.name.as_ref() {
        "MemoryMidiSource" => (
            Symbol::qualified("midi", "MidiSourceFactory"),
            "in-memory source",
        ),
        "MemoryMidiSink" => (
            Symbol::qualified("midi", "MidiSinkFactory"),
            "in-memory sink",
        ),
        "MemoryTrackedMidiSource" => (
            Symbol::qualified("midi", "TrackedMidiSourceFactory"),
            "in-memory tracked source",
        ),
        _ => (Symbol::qualified("midi", "MidiSourceFactory"), "unknown"),
    };
    SurfaceValueSpec {
        symbol: symbol.clone(),
        fields: vec![
            (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
            (Symbol::new("layer"), SurfaceField::Str("midi".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (Symbol::new("shape"), SurfaceField::Symbol(shape)),
            (
                Symbol::new("dependencies"),
                SurfaceField::Strs(vec!["midi-core".to_owned()]),
            ),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (Symbol::new("mode"), SurfaceField::Str(mode.to_owned())),
        ],
    }
}

fn registry_value_spec() -> SurfaceValueSpec {
    SurfaceValueSpec {
        symbol: registry_symbol(),
        fields: vec![
            (
                Symbol::new("symbol"),
                SurfaceField::Symbol(registry_symbol()),
            ),
            (Symbol::new("layer"), SurfaceField::Str("midi".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("shape"),
                SurfaceField::Symbol(Symbol::qualified("midi", "MidiSourceFactory")),
            ),
            (Symbol::new("dependencies"), SurfaceField::Symbols(vec![])),
            (Symbol::new("lossless"), SurfaceField::Bool(true)),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
            (
                Symbol::new("sources"),
                SurfaceField::Symbols(source_symbols()),
            ),
            (Symbol::new("sinks"), SurfaceField::Symbols(sink_symbols())),
            (
                Symbol::new("tracked-sources"),
                SurfaceField::Symbols(tracked_source_symbols()),
            ),
        ],
    }
}

fn midi_io_pack() -> SurfacePackLib {
    let mut values: Vec<SurfaceValueSpec> = io_symbols().into_iter().map(io_value_spec).collect();
    values.push(registry_value_spec());
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(MIDI_IO_LIB_ID),
            values,
        },
    }
}
