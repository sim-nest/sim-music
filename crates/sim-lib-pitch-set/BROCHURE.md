# sim-lib-pitch-set

In one line: Packs a group of pitches into a compact bit pattern and runs the set-theory operations on it fast.

## What it gives you

This models unordered groups of pitches as compact bitmasks. A twelve-slot mask packs the pitch classes into a small integer and supports transposition by rotation, inversion, numeric normalisation, conventional normal-order and prime-form classification, complement and inclusion checks, symmetry scans, Z-relation checks, and the interval-vector census that set theory leans on. Its typed relation report retains every exact Tn/TnI operator and keeps conventional T/TI equivalence, inclusion, common tones, complement classes, interval vectors, and Z-relations distinct. A wider mask does the same across the full 128-key range. Companion types pair a mask with an optional root and encode chords as stacks of thirds, giving analysis tools a quick, exact representation.

## Why you will be glad

- Represent any group of notes as one small, fast value.
- Transpose, invert, and normalise sets with quick operations.
- Classify conventional pitch-set identity without mixing it with numeric mask identity.
- Get the interval-vector census set theory depends on.
- Compare sets with explicit operator and canonical-form evidence.

## Where it fits

This is the set-theory workbench of the SIM pitch family. It underpins the dissonance scorer and the naming schools, which read these masks to classify and label harmony, giving the whole constellation an efficient shared shape for pitch collections.
