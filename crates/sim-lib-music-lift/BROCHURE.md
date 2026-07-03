# sim-lib-music-lift

In one line: Raises a plain MIDI file into something richer -- a readable piano roll, a chord progression, or separated voices.

## What it gives you

Lifting takes a low-level recording and pulls more meaning out of it. This crate reads a parsed MIDI file and raises it into a structured music view: a piano roll, a moment-by-moment analysis roll, a chord progression, or a counterpoint of separated voices. Each lifter reports back not just the result but diagnostics that explain any lossy or ambiguous choices it had to make, so you can trust what you got and see where it guessed. Convenience entry points make each lift a single call.

## Why you will be glad

- Turn a flat MIDI file into a readable chord progression.
- Split a dense track into separate melodic voices.
- See notes about every ambiguous or lossy decision the lifter made.

## Where it fits

This is the upward ramp in the SIM music stack. It bridges the MIDI file world into the structured music model, feeding analysis, naming, and notation tools that expect meaning rather than raw events. It is registered as a runtime library so agents can lift material on demand.
