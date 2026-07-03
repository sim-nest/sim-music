use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_stream_host::HostReconnectPolicy;

/// Bluetooth LE MIDI service UUID from the BLE-MIDI specification.
pub const BLE_MIDI_SERVICE_UUID: &str = "03B80E5A-EDE8-4B33-A751-6CE34EC4C700";

/// Bluetooth LE MIDI I/O characteristic UUID from the BLE-MIDI specification.
pub const BLE_MIDI_IO_CHARACTERISTIC_UUID: &str = "7772E5DB-3868-4112-A1A9-F2669D106BF3";

/// Stable host-backend id for BLE-MIDI discovery.
pub fn ble_midi_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "ble-midi")
}

/// Stable transport id for BLE-MIDI discovery.
pub fn ble_midi_transport_symbol() -> Symbol {
    Symbol::qualified("stream/transport", "ble-midi")
}

/// Reconnect policy used for BLE-MIDI devices by default.
pub fn default_ble_reconnect_policy() -> HostReconnectPolicy {
    HostReconnectPolicy::bounded(8, 250)
}

/// Compatibility names for Yamaha MD-BT01 class BLE-MIDI devices and bridges.
pub fn md_bt01_compatibility_names() -> &'static [&'static str] {
    &["MD-BT01", "UD-BT01", "Yamaha MD-BT01", "Yamaha UD-BT01"]
}

/// A BLE-MIDI endpoint discovered directly or through a documented bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BleMidiDevice {
    id: Symbol,
    name: String,
    address: String,
    gatt_path: Option<String>,
    bridge_port: Option<Symbol>,
    reconnect: HostReconnectPolicy,
}

impl BleMidiDevice {
    /// Creates a device with the default reconnect policy and no bridge port.
    pub fn new(id: impl Into<String>, name: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            id: Symbol::new(id.into()),
            name: name.into(),
            address: address.into(),
            gatt_path: None,
            bridge_port: None,
            reconnect: default_ble_reconnect_policy(),
        }
    }

    /// Returns a deterministic MD-BT01 device fixture for tests and examples.
    pub fn md_bt01_fixture() -> Self {
        Self::new("ble-midi/md-bt01", "Yamaha MD-BT01", "fixture:md-bt01")
    }

    /// Returns a deterministic BLE-MIDI fixture device with a fixture address.
    pub fn fixture(id: impl Into<String>, name: impl Into<String>) -> Self {
        let id = id.into();
        let name = name.into();
        Self::new(id, name, "fixture:ble-midi")
    }

    /// Returns this device tagged with the RtMidi bridge port that exposes it.
    pub fn with_bridge_port(mut self, port: Symbol) -> Self {
        self.bridge_port = Some(port);
        self
    }

    /// Returns this device tagged with the BlueZ GATT characteristic path that exposes it.
    pub fn with_gatt_path(mut self, path: impl Into<String>) -> Self {
        self.gatt_path = Some(path.into());
        self
    }

    /// Returns this device with the given reconnect policy applied.
    pub fn with_reconnect(mut self, reconnect: HostReconnectPolicy) -> Self {
        self.reconnect = reconnect;
        self
    }

    /// Returns the device's stable id symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the human-readable device name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the device address (a hardware address or fixture marker).
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Returns the BlueZ GATT characteristic path, if this device was discovered through BlueZ.
    pub fn gatt_path(&self) -> Option<&str> {
        self.gatt_path.as_deref()
    }

    /// Returns the bridging RtMidi port, if this device is reached through one.
    pub fn bridge_port(&self) -> Option<&Symbol> {
        self.bridge_port.as_ref()
    }

    /// Returns the device's reconnect policy.
    pub fn reconnect(&self) -> &HostReconnectPolicy {
        &self.reconnect
    }

    /// Builds the browse card expression describing this device.
    pub fn card_expr(&self) -> Expr {
        let mut entries = vec![
            (
                Expr::Symbol(Symbol::new("subject")),
                Expr::Symbol(self.id.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("kind")),
                Expr::Symbol(Symbol::qualified("midi", "ble-device")),
            ),
            (
                Expr::Symbol(Symbol::new("backend")),
                Expr::Symbol(ble_midi_backend_symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("transport")),
                Expr::Symbol(ble_midi_transport_symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("name")),
                Expr::String(self.name.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("address")),
                Expr::String(self.address.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("service-uuid")),
                Expr::String(BLE_MIDI_SERVICE_UUID.to_owned()),
            ),
            (
                Expr::Symbol(Symbol::new("io-characteristic-uuid")),
                Expr::String(BLE_MIDI_IO_CHARACTERISTIC_UUID.to_owned()),
            ),
            (
                Expr::Symbol(Symbol::new("reconnect-enabled")),
                Expr::Bool(self.reconnect.enabled()),
            ),
            (
                Expr::Symbol(Symbol::new("reconnect-max-attempts")),
                Expr::Number(integer_expr(self.reconnect.max_attempts())),
            ),
            (
                Expr::Symbol(Symbol::new("reconnect-backoff-ms")),
                Expr::Number(integer_expr(self.reconnect.backoff_ms())),
            ),
        ];
        if let Some(port) = &self.bridge_port {
            entries.push((
                Expr::Symbol(Symbol::new("bridge-port")),
                Expr::Symbol(port.clone()),
            ));
        }
        if let Some(path) = &self.gatt_path {
            entries.push((
                Expr::Symbol(Symbol::new("gatt-path")),
                Expr::String(path.clone()),
            ));
        }
        Expr::Map(entries)
    }
}

fn integer_expr(value: u32) -> NumberLiteral {
    NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    }
}
