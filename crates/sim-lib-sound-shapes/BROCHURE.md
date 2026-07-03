# sim-lib-sound-shapes

In one line: Gives every sound ingredient -- tones, spectra, timbres, tunings -- a readable text form and makes it a runtime object.

## What it gives you

This gives the sound layer a text skin. Its encode and decode functions round-trip the sound types -- frequency, amplitude, phase, partial, envelope, tone, spectrum, timbre, filters, tuning descriptors, dissonance models, and the bridge and renderer options -- through their bracketed text forms. Citizen descriptors wrap those forms as first-class objects, and a runtime surface installs the shape definitions as a library, so a timbre or tuning becomes something you can write, read back, and hand to the runtime.

## Why you will be glad

- Write a timbre or tuning as text and rebuild it exactly.
- Hand sound settings to the runtime as named objects.
- Share synthesis recipes through one canonical form.

## Where it fits

This is the shape and codec surface for the sound family, the counterpart to the music, MIDI, and pitch shape crates. It lets agents and tools view, construct, and match sound objects through the same protocol that governs every other value in SIM.
