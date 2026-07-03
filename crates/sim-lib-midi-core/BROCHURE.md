# sim-lib-midi-core

In one line: The shared vocabulary of MIDI -- notes, timing, and control messages -- that every other music tool in SIM speaks.

## What it gives you

This is the common ground for MIDI across the whole music stack. It defines tick-based timing, the small bounded numbers MIDI uses for bytes and channels, and the event model covering note messages, meta events, and system-exclusive data. It also gives you in-memory ways to read and write streams of events, a simple note-echo transform, controller-number names, and tempo math. Because everything agrees on these types, higher tools can pass music around without translating between private formats.

## Why you will be glad

- Move MIDI between tools without reinventing note and timing types.
- Count time in ticks and convert tempo with math that already works.
- Build on one event model that the whole music stack shares.

## Where it fits

This is the foundation stone of the SIM MIDI world. File readers, live transports, and system-exclusive tooling all build on these types rather than inventing their own, so the constellation stays consistent from the smallest byte up to a full performance.
