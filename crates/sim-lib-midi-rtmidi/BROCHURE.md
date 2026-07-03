# sim-lib-midi-rtmidi

In one line: A bridge to the MIDI ports on your computer, with a stand-in mode for testing when no gear is plugged in.

## What it gives you

This connects the SIM music stack to the ordinary MIDI ports your operating system exposes through the well-known RtMidi backend. By default it serves predictable fake ports, so you can develop and check MIDI routing with nothing physically attached. Switch on the hardware option and it enumerates real ports and opens them, while the opened streams still behave as the same MIDI sources and sinks the rest of the stack expects, including precise conversion of backend timestamps into tick time.

## Why you will be glad

- Reach the MIDI ports already on your machine through one adapter.
- Develop against fake ports first, then flip to real hardware.
- Keep timing exact when backend microseconds become musical ticks.

## Where it fits

This is the wired doorway to hardware in the SIM MIDI world. Where the Bluetooth adapter covers wireless gear, this one covers cabled and virtual ports, feeding the same shared model so nothing downstream needs to know which backend delivered the notes.
