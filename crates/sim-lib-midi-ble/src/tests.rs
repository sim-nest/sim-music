use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
#[cfg(feature = "ble-midi-hardware")]
use sim_lib_midi_core::{
    Channel, ChannelMessage, MidiEvent, MidiPayload, MidiSink, MidiSource, TickTime, U7,
    synthetic_origin,
};
use sim_lib_midi_rtmidi::RtmidiPort;
use sim_lib_stream_host::HostReconnectPolicy;
#[cfg(feature = "ble-midi-hardware")]
use sim_lib_stream_host::{CatalogDeviceProvider, DeviceDirection, DeviceKind, Placement};

use crate::{
    BLE_MIDI_IO_CHARACTERISTIC_UUID, BLE_MIDI_SERVICE_UUID, BleMidiDevice, BluezDeviceFixture,
    detect_external_bridge, discover_bluez_fixtures, install_midi_ble_lib,
    md_bt01_compatibility_names, missing_bluez_dependency_card,
};
#[cfg(feature = "ble-midi-hardware")]
use crate::{
    BleMidiGattSession, BleMidiInputSource, BleMidiOutputSink, BleMidiProvider, DEFAULT_TPQ,
    FixtureBleMidiProvider, decode_ble_midi_packet, encode_ble_midi_event,
};

#[test]
fn bluez_fixture_discovers_ble_midi_uuids() {
    let report = discover_bluez_fixtures(&[BluezDeviceFixture::md_bt01()]);

    assert_eq!(report.devices().len(), 1);
    assert_eq!(report.devices()[0].name(), "Yamaha MD-BT01");
    assert!(report.cards().is_empty());
    assert!(
        report
            .card_exprs()
            .iter()
            .any(|card| field(card, "service-uuid")
                == Some(&Expr::String(BLE_MIDI_SERVICE_UUID.to_owned())))
    );
}

#[test]
fn missing_bluez_dependency_reports_a_card() {
    let report = discover_bluez_fixtures(&[]);

    assert!(report.devices().is_empty());
    assert_eq!(
        field(report.cards().first().unwrap(), "kind"),
        Some(&Expr::Symbol(Symbol::qualified(
            "midi",
            "ble-missing-dependency"
        )))
    );
    assert_eq!(
        field(&missing_bluez_dependency_card(), "io-characteristic-uuid"),
        Some(&Expr::String(BLE_MIDI_IO_CHARACTERISTIC_UUID.to_owned()))
    );
}

#[test]
fn external_bridge_detects_md_bt01_class_rtmidi_port() {
    let port = RtmidiPort::input("rtmidi/md-bt01", "Yamaha MD-BT01 MIDI 1", 2)
        .with_reconnect(HostReconnectPolicy::bounded(3, 100));
    let report = detect_external_bridge(std::slice::from_ref(&port));

    assert_eq!(report.devices().len(), 1);
    assert_eq!(report.devices()[0].bridge_port(), Some(port.id()));
    assert_eq!(report.devices()[0].reconnect().max_attempts(), 3);
    assert!(report.card_exprs().iter().any(|card| field(card, "kind")
        == Some(&Expr::Symbol(Symbol::qualified(
            "midi",
            "ble-operator-path"
        )))));
}

#[test]
fn md_bt01_fixture_has_reconnect_policy_and_operator_names() {
    let device = BleMidiDevice::md_bt01_fixture();

    assert!(device.reconnect().enabled());
    assert!(md_bt01_compatibility_names().contains(&"MD-BT01"));
    assert_eq!(
        field(&device.card_expr(), "kind"),
        Some(&Expr::Symbol(Symbol::qualified("midi", "ble-device")))
    );
}

#[test]
fn install_midi_ble_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_midi_ble_lib,
        &Symbol::new("midi-ble"),
        &[Symbol::qualified("midi", "BleMidiBackend")],
    );
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn ble_midi_timestamp_packet_round_trips_event() {
    let event = note_on_event(129);
    let packet = encode_ble_midi_event(&event).unwrap();

    assert_eq!(packet, vec![0x81, 0x81, 0x90, 60, 100]);

    let events = decode_ble_midi_packet(&packet, DEFAULT_TPQ).unwrap();
    assert_eq!(events, vec![event]);
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn ble_midi_source_and_sink_use_gatt_session() {
    let device = BleMidiDevice::fixture("ble-midi/bluez-0", "BLE MIDI Device 0");
    let session = BleMidiGattSession::fixture(device);
    let event = note_on_event(12);
    let packet = encode_ble_midi_event(&event).unwrap();

    let mut source =
        BleMidiInputSource::from_packet(session.clone(), &packet, DEFAULT_TPQ, 4).unwrap();
    assert_eq!(source.next().unwrap(), Some(event.clone()));
    assert_eq!(source.next().unwrap(), None);

    let mut sink = BleMidiOutputSink::new(session, DEFAULT_TPQ);
    sink.write(&event).unwrap();
    sink.flush().unwrap();
    assert_eq!(sink.session().written_packets(), &[packet]);
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn ble_midi_provider_fixture_enumerates_with_hardware_placement() {
    let fixture = FixtureBleMidiProvider::new(vec![BleMidiDevice::fixture(
        "ble-midi/bluez-0",
        "BLE MIDI Device 0",
    )]);
    let provider = BleMidiProvider::from_discovery(&fixture).unwrap();

    let records = provider.enumerate().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, DeviceKind::Midi);
    assert_eq!(records[0].direction, DeviceDirection::Duplex);
    assert_eq!(
        records[0].placement,
        Placement::Hardware {
            transport: Symbol::new("ble-midi"),
        }
    );
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn bluez_discovery_preserves_empty_discovery_as_missing_dependency() {
    let report = crate::native::bluez_discovery_report(Ok(Vec::new())).unwrap();

    assert!(report.devices().is_empty());
    assert_eq!(
        field(report.cards().first().unwrap(), "kind"),
        Some(&Expr::Symbol(Symbol::qualified(
            "midi",
            "ble-missing-dependency"
        )))
    );
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn bluez_discovery_errors_are_not_collapsed_to_empty_discovery() {
    let error = crate::native::bluez_discovery_report(Err(sim_kernel::Error::Eval(
        "D-Bus permission denied".to_owned(),
    )))
    .unwrap_err();

    assert!(format!("{error}").contains("D-Bus permission denied"));
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn ble_midi_provider_surfaces_discovery_provider_errors() {
    let error = BleMidiProvider::from_discovery(&FailingBleMidiProvider).unwrap_err();

    assert!(format!("{error}").contains("provider unavailable"));
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn ble_midi_provider_fixture_opens_duplex_eval_site() {
    let fixture = FixtureBleMidiProvider::new(vec![BleMidiDevice::fixture(
        "ble-midi/bluez-0",
        "BLE MIDI Device 0",
    )]);
    let provider = BleMidiProvider::from_discovery(&fixture).unwrap();

    let site = provider.open(&Symbol::new("ble-midi/bluez-0")).unwrap();
    assert_eq!(
        site.placement(),
        &Placement::Hardware {
            transport: Symbol::new("ble-midi"),
        }
    );
    assert_eq!(site.device_record().direction, DeviceDirection::Duplex);
    site.close().unwrap();
}

#[cfg(all(feature = "ble-midi-hardware", target_os = "linux"))]
#[test]
fn ble_midi_bluez_gatt_smoke() {
    if std::env::var("SIM_BLE_MIDI_HARDWARE_SMOKE").as_deref() != Ok("1") {
        eprintln!("set SIM_BLE_MIDI_HARDWARE_SMOKE=1 to open a BLE-MIDI device");
        return;
    }

    let device_id =
        std::env::var("SIM_BLE_MIDI_DEVICE").unwrap_or_else(|_| "ble-midi/bluez-0".to_owned());
    let discovery = crate::BluezBleMidiProvider::default();
    let provider = BleMidiProvider::from_discovery(&discovery).unwrap();
    let site = provider.open(&Symbol::new(device_id)).unwrap();
    site.close().unwrap();
}

fn field<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == name => {
            Some(value)
        }
        _ => None,
    })
}

#[cfg(feature = "ble-midi-hardware")]
fn note_on_event(ticks: i64) -> MidiEvent {
    MidiEvent {
        time: TickTime::new(ticks, DEFAULT_TPQ).unwrap(),
        origin: synthetic_origin(),
        payload: MidiPayload::Channel(ChannelMessage::NoteOn {
            ch: Channel::new(0).unwrap(),
            key: U7(60),
            vel: U7(100),
        }),
    }
}

#[cfg(feature = "ble-midi-hardware")]
struct FailingBleMidiProvider;

#[cfg(feature = "ble-midi-hardware")]
impl crate::BleMidiDiscoveryProvider for FailingBleMidiProvider {
    fn discover(&self) -> sim_kernel::Result<crate::BleMidiDiscoveryReport> {
        Err(sim_kernel::Error::Eval("provider unavailable".to_owned()))
    }
}
