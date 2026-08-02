# sim-lib-music-serial

In one line: keeps serial plans immutable, explicit, and honest through realization.

## What it gives you

This library is the structural source for serial practice. A plan retains exact
row identity, stable event identity, role/origin provenance, parent evidence,
voice identity, simultaneous groups for chords, and a validated precedence DAG
for what must happen before what. It never fabricates a total onset order just
to make a chord easy to store.

## Why you will be glad

- Keep one row statement visible even when it spans several voices and vertical sonorities.
- Evaluate serial practice through explicit invariant ledgers instead of a hidden strict/loose switch.
- Add strict realization choices without losing source-plan or ordinal provenance.

## Where it fits

Use this when ornaments, derived material, modal landing, completion, or
counter-voices must still point back to one immutable serial source. The plan
validates row and event references, structural coverage, parent evidence, and
cycles up front; `SerialPractice` evaluates named readings through open
`PracticeRule` components; and strict realization preserves that provenance
through canonical music-core staff, piano-roll, and score rendering.
