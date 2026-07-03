# BLE MIDI smoke

This recipe records the operator command for opening a BLE MIDI device through
the hardware feature:

```bash
SIM_BLE_MIDI_HARDWARE_SMOKE=1 SIM_BLE_MIDI_DEVICE=ble-midi/bluez-0 \
  cargo test -p sim-lib-midi-ble --features ble-midi-hardware \
  -- --nocapture ble_midi_bluez_gatt_smoke
```

Normal validation leaves the environment variable unset, so the smoke test
prints the required setup and exits without touching Bluetooth state.
