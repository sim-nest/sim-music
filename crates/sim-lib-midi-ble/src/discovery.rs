use sim_kernel::{Expr, Symbol};
use sim_lib_midi_rtmidi::RtmidiPort;

use crate::{
    BLE_MIDI_IO_CHARACTERISTIC_UUID, BLE_MIDI_SERVICE_UUID, BleMidiDevice, ble_midi_backend_symbol,
    md_bt01_compatibility_names,
};

/// BlueZ-style device data used by deterministic discovery tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BluezDeviceFixture {
    name: String,
    address: String,
    service_uuids: Vec<String>,
    characteristic_uuids: Vec<String>,
}

/// Result of a BLE-MIDI discovery pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BleMidiDiscoveryReport {
    devices: Vec<BleMidiDevice>,
    cards: Vec<Expr>,
}

impl BluezDeviceFixture {
    /// Creates a fixture from advertised service and characteristic UUIDs.
    pub fn new(
        name: impl Into<String>,
        address: impl Into<String>,
        service_uuids: Vec<String>,
        characteristic_uuids: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            address: address.into(),
            service_uuids,
            characteristic_uuids,
        }
    }

    /// Returns a fixture advertising the BLE-MIDI UUIDs of an MD-BT01 device.
    pub fn md_bt01() -> Self {
        Self::new(
            "Yamaha MD-BT01",
            "00:1D:43:AA:BB:CC",
            vec![BLE_MIDI_SERVICE_UUID.to_owned()],
            vec![BLE_MIDI_IO_CHARACTERISTIC_UUID.to_owned()],
        )
    }

    /// Returns the advertised device name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the device address.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl BleMidiDiscoveryReport {
    /// Creates a report from discovered devices and any standalone cards.
    pub fn new(devices: Vec<BleMidiDevice>, cards: Vec<Expr>) -> Self {
        Self { devices, cards }
    }

    /// Returns the discovered devices.
    pub fn devices(&self) -> &[BleMidiDevice] {
        &self.devices
    }

    /// Returns the standalone cards (for example missing-dependency notices).
    pub fn cards(&self) -> &[Expr] {
        &self.cards
    }

    /// Returns every card: one per device plus the standalone cards.
    pub fn card_exprs(&self) -> Vec<Expr> {
        self.devices
            .iter()
            .map(BleMidiDevice::card_expr)
            .chain(self.cards.iter().cloned())
            .collect()
    }
}

/// Discovers BLE-MIDI endpoints from BlueZ-style device data.
pub fn discover_bluez_fixtures(fixtures: &[BluezDeviceFixture]) -> BleMidiDiscoveryReport {
    let devices = fixtures
        .iter()
        .filter(|fixture| fixture.has_ble_midi_uuids())
        .enumerate()
        .map(|(index, fixture)| {
            BleMidiDevice::new(
                format!("ble-midi/bluez-{index}"),
                fixture.name.clone(),
                fixture.address.clone(),
            )
        })
        .collect::<Vec<_>>();
    let cards = if devices.is_empty() {
        vec![missing_bluez_dependency_card()]
    } else {
        Vec::new()
    };
    BleMidiDiscoveryReport::new(devices, cards)
}

/// Detects documented external BLE-MIDI bridges exposed as RtMidi ports.
pub fn detect_external_bridge(ports: &[RtmidiPort]) -> BleMidiDiscoveryReport {
    let devices = ports
        .iter()
        .filter(|port| is_md_bt01_class_name(port.name()))
        .map(|port| {
            BleMidiDevice::new(
                format!("ble-midi/bridge-{}", port.index()),
                port.name().to_owned(),
                "external-rtmidi-bridge",
            )
            .with_bridge_port(port.id().clone())
            .with_reconnect(port.reconnect().clone())
        })
        .collect::<Vec<_>>();
    let mut cards = vec![operator_path_card()];
    if devices.is_empty() {
        cards.push(missing_bluez_dependency_card());
    }
    BleMidiDiscoveryReport::new(devices, cards)
}

/// Structured card for hosts without usable BlueZ BLE-MIDI discovery.
pub fn missing_bluez_dependency_card() -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("subject")),
            Expr::Symbol(ble_midi_backend_symbol()),
        ),
        (
            Expr::Symbol(Symbol::new("kind")),
            Expr::Symbol(Symbol::qualified("midi", "ble-missing-dependency")),
        ),
        (
            Expr::Symbol(Symbol::new("dependency")),
            Expr::String("BlueZ D-Bus BLE-MIDI discovery or external BLE-MIDI bridge".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("service-uuid")),
            Expr::String(BLE_MIDI_SERVICE_UUID.to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("io-characteristic-uuid")),
            Expr::String(BLE_MIDI_IO_CHARACTERISTIC_UUID.to_owned()),
        ),
    ])
}

/// Operator-facing card for MD-BT01 class devices.
pub fn operator_path_card() -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("subject")),
            Expr::Symbol(Symbol::qualified("midi", "MD-BT01")),
        ),
        (
            Expr::Symbol(Symbol::new("kind")),
            Expr::Symbol(Symbol::qualified("midi", "ble-operator-path")),
        ),
        (
            Expr::Symbol(Symbol::new("device-class")),
            Expr::String("Yamaha MD-BT01 class BLE-MIDI".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("path")),
            Expr::String(
                "Discover BLE-MIDI UUIDs through BlueZ or pair through a documented BLE-MIDI-to-ALSA bridge, then open the resulting RtMidi port."
                    .to_owned(),
            ),
        ),
    ])
}

impl BluezDeviceFixture {
    fn has_ble_midi_uuids(&self) -> bool {
        has_uuid(&self.service_uuids, BLE_MIDI_SERVICE_UUID)
            && has_uuid(&self.characteristic_uuids, BLE_MIDI_IO_CHARACTERISTIC_UUID)
    }
}

fn is_md_bt01_class_name(name: &str) -> bool {
    let normalized = name.to_ascii_uppercase();
    md_bt01_compatibility_names()
        .iter()
        .any(|candidate| normalized.contains(&candidate.to_ascii_uppercase()))
}

fn has_uuid(uuids: &[String], expected: &str) -> bool {
    uuids.iter().any(|uuid| uuid.eq_ignore_ascii_case(expected))
}
