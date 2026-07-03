# sim-lib-sound-wasm-frame

In one line: Frame-safe descriptions of sound data, ready to carry across to a browser or plugin boundary.

## What it gives you

This packages sound information into simple frames that travel cleanly across an interface boundary. Despite the name it compiles no WebAssembly and ships no browser glue; it defines frame-safe sound descriptors and the stable string names that wasm-engine entrypoints go by, so a browser or plugin adapter has fixed identifiers to bind against. It carries no compiled engine and runs nothing itself -- it is the settled shape sound data takes at the edge.

## Why you will be glad

- Pass sound data across a plugin or browser edge intact.
- Bind to stable entrypoint names that stay fixed.
- Keep the boundary light, with descriptors instead of an engine.

## Where it fits

This is the hand-off layer between the SIM sound core and the surfaces that embed it. Alongside the music, MIDI, and pitch frame crates, it gives web and plugin adapters a common frame format so sound can leave the Rust core without exposing its inner model.
