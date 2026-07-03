# sim-lib-pitch-shapes

In one line: Gives pitches, scales, chords, and keys a readable text form and makes them objects the SIM runtime can hold.

## What it gives you

This is the text and runtime surface for the pitch theory tools. It provides string round-trips for pitches, intervals, pitch-class masks, scales, keys, chords, and chord symbols, wraps each canonical form in a citizen descriptor for read-and-construct evaluation, and exposes the types as SIM shapes through a loadable library. In short, a scale or chord becomes something you can write down, read back exactly, and hand to the runtime as a named object.

## Why you will be glad

- Write a scale or chord as text and rebuild it precisely.
- Let the runtime match and construct pitch values directly.
- Share pitch material through one canonical text form.

## Where it fits

This is the shape and codec surface for the pitch family, the counterpart to the music, MIDI, and sound shape crates. It lets agents and tools view, construct, and pattern-match pitch objects through the same protocol that governs every other value in SIM.
