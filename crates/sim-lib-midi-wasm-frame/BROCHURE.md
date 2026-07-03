# sim-lib-midi-wasm-frame

In one line: Compact, frame-safe descriptions of MIDI data ready to hand across to a browser or plugin boundary.

## What it gives you

This packages MIDI into simple, self-contained frames that survive a trip across an interface boundary. Despite the name it does not compile any WebAssembly or ship browser glue; instead it defines the data descriptors that a web or plugin adapter can serialise and pass along, and each event frame round-trips exactly through its byte form. It is the neutral shape MIDI takes when it needs to leave the Rust core and be received elsewhere.

## Why you will be glad

- Ship MIDI events across a plugin or browser boundary without loss.
- Keep the byte layout stable, so both sides always agree.
- Depend only on plain descriptors, with no engine baked in.

## Where it fits

This is the hand-off layer between the SIM MIDI core and the outside surfaces that host it. It gives web and plugin adapters a settled frame format to bind to, so the browser bridge and other embeddings can carry MIDI without reaching into the internal model.
