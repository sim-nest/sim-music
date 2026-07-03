# sim-lib-pitch-core

In one line: The basic alphabet of pitch -- notes, note names, octaves, and the distances between them -- that the rest of the theory tools share.

## What it gives you

This defines the foundation pitch types every other pitch crate builds on: the twelve pitch classes counted mod-12, octave-aware pitches, spelled notes with letter and accidental, and intervals measured in semitones. It follows the familiar conventions where C is zero and middle C is MIDI note 60, so the numbers line up with what musicians and MIDI both expect. Higher crates lean on these types instead of inventing their own.

## Why you will be glad

- Name a note, its octave, and its spelling with settled types.
- Measure the distance between any two pitches in semitones.
- Trust that C-is-zero and middle-C conventions match MIDI.

## Where it fits

This is the bedrock of the SIM pitch family. Sets, scales, chords, and the naming schools all rest on these primitives, so every theory tool in the constellation counts pitches and intervals the same way.
