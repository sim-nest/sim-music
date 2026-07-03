# sim-lib-pitch-wasm-frame

In one line: Frame-safe descriptions of pitch data, ready to carry across to a browser or plugin boundary.

## What it gives you

This packages pitch data into simple, self-contained frames that survive a trip across an interface boundary. Despite the name it compiles no WebAssembly and ships no browser glue; it defines the data descriptors that a web or plugin adapter can serialise and pass along. It is the neutral shape pitch information takes when it needs to leave the Rust core and be received by some other surface.

## Why you will be glad

- Pass pitch values across a plugin or browser edge intact.
- Keep the frame layout stable so both sides agree.
- Depend on plain descriptors, with no engine attached.

## Where it fits

This is the hand-off layer between the SIM pitch core and the surfaces that host it. Alongside the music, MIDI, and sound frame crates, it gives web and plugin adapters a settled frame format so pitch information can leave the core without exposing its inner types.
