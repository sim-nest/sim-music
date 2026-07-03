use sim_kernel::{Error, Result};

use crate::{BLE_MIDI_IO_CHARACTERISTIC_UUID, BLE_MIDI_SERVICE_UUID, BleMidiDevice};

#[cfg(target_os = "linux")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct BluezDeviceInfo {
    path: String,
    name: String,
    address: String,
    service_uuids: Vec<String>,
}

#[cfg(target_os = "linux")]
pub(super) fn discover_bluez_dbus() -> Result<Vec<BleMidiDevice>> {
    use zbus::blocking::{Connection, fdo::ObjectManagerProxy};

    let connection = Connection::system().map_err(bluez_error)?;
    let manager = ObjectManagerProxy::builder(&connection)
        .destination("org.bluez")
        .map_err(bluez_error)?
        .path("/")
        .map_err(bluez_error)?
        .build()
        .map_err(bluez_error)?;
    let objects = manager.get_managed_objects().map_err(bluez_error)?;
    Ok(devices_from_bluez_objects(&objects))
}

#[cfg(not(target_os = "linux"))]
pub(super) fn discover_bluez_dbus() -> Result<Vec<BleMidiDevice>> {
    Err(Error::Eval(
        "BlueZ BLE-MIDI discovery is available on Linux".to_owned(),
    ))
}

#[cfg(target_os = "linux")]
fn devices_from_bluez_objects(objects: &zbus::fdo::ManagedObjects) -> Vec<BleMidiDevice> {
    let device_infos = objects
        .iter()
        .filter_map(|(path, interfaces)| {
            interfaces
                .iter()
                .find(|(interface, _)| interface.to_string() == "org.bluez.Device1")
                .map(|(_, properties)| BluezDeviceInfo {
                    path: path.to_string(),
                    name: property_string(properties, "Name")
                        .or_else(|| property_string(properties, "Alias"))
                        .unwrap_or_else(|| "BLE MIDI Device".to_owned()),
                    address: property_string(properties, "Address")
                        .unwrap_or_else(|| path.to_string()),
                    service_uuids: property_string_vec(properties, "UUIDs").unwrap_or_default(),
                })
        })
        .collect::<Vec<_>>();

    let mut devices = Vec::new();
    for (path, interfaces) in objects {
        let Some((_, properties)) = interfaces
            .iter()
            .find(|(interface, _)| interface.to_string() == "org.bluez.GattCharacteristic1")
        else {
            continue;
        };
        let Some(uuid) = property_string(properties, "UUID") else {
            continue;
        };
        if !uuid.eq_ignore_ascii_case(BLE_MIDI_IO_CHARACTERISTIC_UUID) {
            continue;
        }
        let Some(device) = device_for_characteristic(&device_infos, &path.to_string()) else {
            continue;
        };
        let advertises_service = device
            .service_uuids
            .iter()
            .any(|uuid| uuid.eq_ignore_ascii_case(BLE_MIDI_SERVICE_UUID));
        if !device.service_uuids.is_empty() && !advertises_service {
            continue;
        }
        devices.push(
            BleMidiDevice::new(
                format!("ble-midi/bluez-{}", devices.len()),
                device.name.clone(),
                device.address.clone(),
            )
            .with_gatt_path(path.to_string()),
        );
    }
    devices
}

#[cfg(target_os = "linux")]
fn device_for_characteristic<'a>(
    devices: &'a [BluezDeviceInfo],
    characteristic_path: &str,
) -> Option<&'a BluezDeviceInfo> {
    devices
        .iter()
        .filter(|device| characteristic_path.starts_with(&format!("{}/", device.path)))
        .max_by_key(|device| device.path.len())
}

#[cfg(target_os = "linux")]
fn property_string(
    properties: &std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    name: &str,
) -> Option<String> {
    properties
        .get(name)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| String::try_from(value).ok())
}

#[cfg(target_os = "linux")]
fn property_string_vec(
    properties: &std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    name: &str,
) -> Option<Vec<String>> {
    properties
        .get(name)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| Vec::<String>::try_from(value).ok())
}

#[cfg(target_os = "linux")]
pub(super) fn write_bluez_characteristic(path: &str, packet: &[u8]) -> Result<()> {
    use std::collections::HashMap;

    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::Value;

    let connection = Connection::system().map_err(bluez_error)?;
    let proxy = Proxy::new(
        &connection,
        "org.bluez",
        path,
        "org.bluez.GattCharacteristic1",
    )
    .map_err(bluez_error)?;
    let options = HashMap::<&str, Value<'_>>::new();
    proxy
        .call::<_, _, ()>("WriteValue", &(packet.to_vec(), options))
        .map_err(bluez_error)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn write_bluez_characteristic(_path: &str, _packet: &[u8]) -> Result<()> {
    Err(Error::Eval(
        "BlueZ BLE-MIDI characteristic writes are available on Linux".to_owned(),
    ))
}

#[cfg(target_os = "linux")]
fn bluez_error(error: impl std::fmt::Display) -> Error {
    Error::Eval(format!("BlueZ BLE-MIDI discovery failed: {error}"))
}
