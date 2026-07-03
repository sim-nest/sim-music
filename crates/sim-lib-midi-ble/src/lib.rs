#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! BLE-MIDI discovery, packet framing, and stream-host placement support.
//!
//! The default `model` feature keeps validation deterministic: discovery can run
//! from BlueZ-style fixture data and documented bridge names without requiring a
//! Bluetooth adapter. The opt-in `ble-midi-hardware` feature exposes BlueZ
//! discovery provider seams, BLE-MIDI timestamp packet framing, source/sink
//! adapters, and a stream-host `DeviceProvider` for hardware placement rows.
//!
//! Platform access stays outside normal validation. Operator-gated smoke tests
//! use `SIM_BLE_MIDI_HARDWARE_SMOKE=1`; hardware-free tests use fixture
//! providers and the same packet/source/sink contracts.
//!
//! # Examples
//!
//! Discovering a BLE-MIDI endpoint from fixture data:
//!
//! ```
//! use sim_lib_midi_ble::{discover_bluez_fixtures, BluezDeviceFixture};
//!
//! let report = discover_bluez_fixtures(&[BluezDeviceFixture::md_bt01()]);
//! assert_eq!(report.devices().len(), 1);
//! assert_eq!(report.devices()[0].name(), "Yamaha MD-BT01");
//! ```

mod discovery;
mod model;
#[cfg(feature = "ble-midi-hardware")]
mod native;
mod runtime;

pub use discovery::{
    BleMidiDiscoveryReport, BluezDeviceFixture, detect_external_bridge, discover_bluez_fixtures,
    missing_bluez_dependency_card, operator_path_card,
};
pub use model::{
    BLE_MIDI_IO_CHARACTERISTIC_UUID, BLE_MIDI_SERVICE_UUID, BleMidiDevice, ble_midi_backend_symbol,
    ble_midi_transport_symbol, default_ble_reconnect_policy, md_bt01_compatibility_names,
};
#[cfg(feature = "ble-midi-hardware")]
pub use native::{
    BleMidiDiscoveryProvider, BleMidiDuplexEvalSite, BleMidiGattSession, BleMidiHardwareConfig,
    BleMidiInputSource, BleMidiOutputSink, BleMidiProvider, BluezBleMidiProvider,
    DEFAULT_RING_CAPACITY, DEFAULT_TPQ, FixtureBleMidiProvider, decode_ble_midi_packet,
    encode_ble_midi_event,
};
pub use runtime::{MidiBleLib, install_midi_ble_lib};

#[cfg(test)]
mod tests;
