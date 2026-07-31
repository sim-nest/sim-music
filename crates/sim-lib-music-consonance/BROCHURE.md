# sim-lib-music-consonance

In one line: Explains how every exact score window sounds without throwing away
notes, identities, timing, or the differences between consonance models.

## What it gives you

This library splits a canonical score, identity-bearing staff, or fully realized
MIDI timeline at every exact onset and release. Each half-open window retains
duplicate pitches as distinct events together with voice, note, event, onset,
release, velocity, channel, articulation, and source evidence.

Each window reports pitch, acoustic, exact-ratio, commonality, and voice-leading
metrics separately. Roughness mass and normalized density remain different named
components. There is no default weighted average hiding which model made a
musical judgment.

The loadable `music/consonance/evaluate` function returns the same structure as
Lisp data, including exact rational spans and every retained identity.

## Why you will be glad

- Slice event-aligned harmony without float drift or boundary ambiguity.
- Keep unisons and doubled notes instead of collapsing them into a pitch set.
- Compare score and MIDI realization through the same metric families.
- Inspect why a window scored as it did before using that evidence in a search.
- Change one explicit model policy without silently changing the other metrics.

## Where it fits

This is the orchestration layer over `sim-lib-music-core`,
`sim-lib-music-lift`, `sim-lib-pitch-dissonance`,
`sim-lib-sound-dissonance`, and `sim-lib-sound-tuning`. Those crates continue to
own exact score identity, MIDI performance semantics, contextual pitch models,
psychoacoustic curves, and tuning. This crate owns their event-window
composition and the inspectable report.
