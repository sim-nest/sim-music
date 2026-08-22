use crate::*;
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_midi_rtmidi::RtmidiPort;
use sim_lib_stream_host::HostReconnectPolicy;
use sim_value::access::field;
use std::sync::Arc;

#[test]
fn fixture_discovery_and_bridge_policy_remain_portable() {
    let report = discover_bluez_fixtures(&[BluezDeviceFixture::md_bt01()]);
    assert_eq!(report.devices()[0].name(), "Yamaha MD-BT01");
    let port = RtmidiPort::input("rtmidi/md-bt01", "Yamaha MD-BT01 MIDI 1", 2)
        .with_reconnect(HostReconnectPolicy::bounded(3, 100));
    assert_eq!(
        detect_external_bridge(&[port]).devices()[0]
            .reconnect()
            .max_attempts(),
        3
    );
    assert_eq!(
        field(&missing_bluez_dependency_card(), "io-characteristic-uuid"),
        Some(&Expr::String(BLE_MIDI_IO_CHARACTERISTIC_UUID.to_owned()))
    );
}

#[test]
fn midi_ble_runtime_exports_only_semantic_backend() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_midi_ble_lib,
        &Symbol::new("midi-ble"),
        &[Symbol::qualified("midi", "BleMidiBackend")],
    );
}
