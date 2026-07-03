use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{ble_midi_backend_symbol, ble_midi_transport_symbol};

const MIDI_BLE_LIB_ID: &str = "midi-ble";

/// Host-registered lib exporting the BLE-MIDI cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct MidiBleLib;

impl Lib for MidiBleLib {
    fn manifest(&self) -> LibManifest {
        ble_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        ble_pack().load(cx, linker)
    }
}

/// Installs [`MidiBleLib`] into `cx` once.
pub fn install_midi_ble_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &MidiBleLib)?;
    Ok(())
}

fn ble_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("midi", "BleMidiBackend"),
        Symbol::qualified("midi", "BleMidiMissingBlueZ"),
        Symbol::qualified("midi", "MDBT01OperatorPath"),
    ]
}

fn ble_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let role = match symbol.name.as_ref() {
        "BleMidiBackend" => "BLE-MIDI discovery backend card",
        "BleMidiMissingBlueZ" => "BLE-MIDI missing BlueZ dependency card",
        "MDBT01OperatorPath" => "MD-BT01 class operator path card",
        _ => "BLE-MIDI card",
    };
    SurfaceValueSpec {
        symbol: symbol.clone(),
        fields: vec![
            (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
            (Symbol::new("layer"), SurfaceField::Str("midi".to_owned())),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("backend"),
                SurfaceField::Symbol(ble_midi_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(ble_midi_transport_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("dependencies"),
                SurfaceField::Strs(vec![
                    "BlueZ D-Bus or BLE-MIDI bridge".to_owned(),
                    "midi-rtmidi".to_owned(),
                    "stream-host".to_owned(),
                ]),
            ),
            (Symbol::new("capabilities"), SurfaceField::Symbols(vec![])),
        ],
    }
}

fn ble_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(MIDI_BLE_LIB_ID),
            values: ble_symbols().into_iter().map(ble_value_spec).collect(),
        },
    }
}
