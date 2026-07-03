use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};
use sim_lib_stream_host::{HostBackendCapability, missing_capability_card_expr};

use crate::{rtmidi_backend_symbol, rtmidi_transport_symbol};

const MIDI_RTMIDI_LIB_ID: &str = "midi-rtmidi";

/// Host-registered lib exporting the RtMidi host-MIDI cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct MidiRtmidiLib;

impl Lib for MidiRtmidiLib {
    fn manifest(&self) -> LibManifest {
        rtmidi_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        rtmidi_pack().load(cx, linker)
    }
}

/// Installs [`MidiRtmidiLib`] into `cx` once.
pub fn install_midi_rtmidi_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &MidiRtmidiLib)?;
    Ok(())
}

/// Returns the browse card describing the missing RtMidi provider dependency.
pub fn missing_rtmidi_dependency_card() -> sim_kernel::Expr {
    missing_capability_card_expr(&rtmidi_backend_symbol(), HostBackendCapability::MidiInput)
}

fn rtmidi_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("midi", "RtMidiBackend"),
        Symbol::qualified("midi", "RtMidiMissingDependency"),
    ]
}

fn rtmidi_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let role = match symbol.name.as_ref() {
        "RtMidiBackend" => "RtMidi host MIDI backend card",
        "RtMidiMissingDependency" => "RtMidi missing dependency card",
        _ => "RtMidi card",
    };
    SurfaceValueSpec {
        symbol: symbol.clone(),
        fields: vec![
            (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
            (Symbol::new("layer"), SurfaceField::Str("midi".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("backend"),
                SurfaceField::Symbol(rtmidi_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(rtmidi_transport_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("dependencies"),
                SurfaceField::Strs(vec![
                    "RtMidi or compatible provider".to_owned(),
                    "midi-core".to_owned(),
                    "stream-host".to_owned(),
                ]),
            ),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
        ],
    }
}

fn rtmidi_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(MIDI_RTMIDI_LIB_ID),
            values: rtmidi_symbols()
                .into_iter()
                .map(rtmidi_value_spec)
                .collect(),
        },
    }
}
