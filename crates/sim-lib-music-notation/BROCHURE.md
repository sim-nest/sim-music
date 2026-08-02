# sim-lib-music-notation

In one line: Moves canonical SIM scores through readable LilyPond and bounded, security-reviewed MusicXML partwise notation.

## What it gives you

This is the notation surface for SIM music. It converts between the canonical
`Score` and two deliberately bounded exchange forms: a human-writable LilyPond
subset and a MusicXML 4.0 `score-partwise` profile. MusicXML parsing rejects
DTDs, entity declarations, unknown extensions, and resource overruns. Stable
part/event ids travel in the report sidecar, and every accepted loss is named
without introducing another score model.

The loadable surface is one Shape-described `music/notation/import` function.
Its result carries the existing `music/Score` citizen read-construct, retained
ids, and losses; MusicXML is a profile of the notation organ, not a new codec
family.

## Why you will be glad

- Write a score as text people who know notation can read.
- Exchange exact monophonic or named-part scores with MusicXML applications.
- Keep source ids stable across import and re-export.
- Get machine-readable diagnostics for every rejected or lossy notation fact.
- Bound bytes, nodes, depth, text, parts, and events before untrusted input can
  become an unbounded workload.

## Where it fits

This is the written-notation organ of the SIM music family. It composes the
existing canonical `Score`, music Shapes, and citizen read-construct surface.
See `MUSICXML_PROFILE.md` for the exact accepted grammar, loss contract, limits,
and XML dependency review.
