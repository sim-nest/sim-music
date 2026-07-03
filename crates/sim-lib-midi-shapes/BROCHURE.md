# sim-lib-midi-shapes

In one line: A tidy text form for MIDI so events, tracks, and whole files can be written down, read back, and handled as first-class objects.

## What it gives you

This gives MIDI a readable text skin. Its encode and decode functions round-trip events, channel messages, meta events, system-exclusive data, raw bytes, tracks, and full Standard MIDI Files through a compact bracketed form, reporting any trouble as clear errors. It also wraps each canonical form as a runtime citizen, so a MIDI note or track becomes an object the SIM runtime can hold, name, and reason about, published under a documented set of shape values.

## Why you will be glad

- Write a MIDI event as short text and get the exact event back.
- Hand MIDI values to the runtime as named, inspectable objects.
- Catch malformed input with plain error messages, not silent guesses.

## Where it fits

This is the codec and object surface for MIDI within SIM. It turns the raw MIDI data model into text you can read and into runtime citizens the shape protocol understands, letting agents and tools view, match, and construct MIDI the same way they handle every other SIM value.
