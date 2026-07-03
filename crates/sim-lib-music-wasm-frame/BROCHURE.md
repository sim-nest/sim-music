# sim-lib-music-wasm-frame

In one line: Frame-safe descriptions of music objects, ready to carry across to a browser or plugin without dragging the whole engine along.

## What it gives you

This packages music into simple frames that travel cleanly across an interface boundary. Despite the name it compiles no WebAssembly and ships no browser glue; it defines frame-safe music descriptors and the stable string names that wasm-engine entrypoints go by, so a browser or plugin adapter has fixed identifiers to bind against. It carries no compiled engine and runs nothing itself -- it is purely the settled shape music takes at the edge.

## Why you will be glad

- Pass music objects across a plugin or browser edge intact.
- Bind to stable entrypoint names that will not shift under you.
- Keep the boundary light, with descriptors instead of an engine.

## Where it fits

This is the hand-off layer between the SIM music core and the surfaces that embed it. Alongside the MIDI, pitch, and sound frame crates, it gives web and plugin adapters a common frame format so music can leave the Rust core without exposing its inner model.
