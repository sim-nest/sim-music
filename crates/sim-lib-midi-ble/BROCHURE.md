# sim-lib-midi-ble

In one line: A way to find and talk to wireless Bluetooth MIDI controllers, or to rehearse that link from saved fixtures when no adapter is around.

## What it gives you

This handles the fiddly parts of Bluetooth Low Energy MIDI: spotting nearby devices, framing the timestamped packets they send, and wiring them in as ordinary MIDI sources and sinks. By default it runs from recorded device fixtures, so discovery can be exercised and checked without a live Bluetooth radio. Turn on the hardware option and it reaches real BlueZ adapters and places wireless endpoints as playable rows in a stream host.

## Why you will be glad

- Bring a wireless keyboard into your setup as a normal MIDI port.
- Rehearse discovery logic from saved device data, no radio needed.
- Keep hardware access behind an explicit switch, so tests stay clean.

## Where it fits

In the SIM music constellation this is one of the doorways to real-world gear -- the wireless one. It feeds the shared MIDI model from Bluetooth controllers, sitting alongside the wired and virtual MIDI adapters so the rest of the stack does not care how the notes arrived.
