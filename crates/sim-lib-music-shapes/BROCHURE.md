# sim-lib-music-shapes

In one line: Gives every music object a readable text form and makes it a first-class object the SIM runtime can hold and match.

## What it gives you

This applies the SIM shape protocol to the music types. It provides read-and-construct citizen descriptors that round-trip music objects through canonical text forms, a bracketed codec that encodes and decodes every music representation, and a loadable library that registers documented shape values for the music namespace. The codec is the canonical text bridge: encode functions render a music value to its text form, and decode functions parse that text straight back into the matching type.

## Why you will be glad

- Write any music object as text and rebuild it exactly.
- Let the runtime match and dispatch on music values directly.
- Share music through one canonical form the whole system reads.

## Where it fits

This is the shape and codec surface for the core music model, the counterpart to the MIDI, pitch, and sound shape crates. It lets agents and tools view, construct, and pattern-match music objects using the same protocol they use for every other value in SIM.
