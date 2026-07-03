# sim-lib-music-notation

In one line: Turns a score into readable notation text and reads it back, using a familiar LilyPond-style form.

## What it gives you

This is the notation surface for SIM music. It converts between a score -- and related pieces such as melodies, progressions, and counterpoint -- and a text rendering in a subset of the well-known LilyPond notation language. A single codec entry point offers import and export in both plain and diagnostic-carrying forms, so you can move fluidly between a music object and human-writable notation, and it installs as a loadable runtime library for on-demand use.

## Why you will be glad

- Write a score as text people who know notation can read.
- Import notation back into a live music object.
- See diagnostics when a passage does not translate cleanly.

## Where it fits

This is the written-notation codec of the SIM music family. It gives scores a page-facing form alongside the MIDI and internal text surfaces, so a piece can travel between the runtime, a MIDI file, and human-readable notation without losing its identity.
