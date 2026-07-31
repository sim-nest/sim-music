# sim-lib-music-consonance

In one line: Explains exact score-window sonance and proposes reversible additions without losing musical evidence.

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

For completion, callers supply typed note, ornament, chord, pedal, doubling, and
voice candidates together with named per-window metric thresholds, protected
spans and identities, pitch ranges, style limits, and ordinary
`SearchControl`. The selected `ConsonancePatch` is bound to the source staff's
kernel content identity. Applying it only adds material; removing it verifies
every added value before restoring the exact source, including all ids.

## Why you will be glad

- Slice event-aligned harmony without float drift or boundary ambiguity.
- Keep unisons and doubled notes instead of collapsing them into a pitch set.
- Compare score and MIDI realization through the same metric families.
- Inspect why a window scored as it did before using that evidence in a search.
- Change one explicit model policy without silently changing the other metrics.
- Keep useful partial completions while retaining an honest partial or cancelled
  search receipt.
- Prove `remove(apply(source, patch), patch) == source` instead of trusting an
  informal undo convention.

## Where it fits

This is the orchestration layer over `sim-lib-music-core`,
`sim-lib-music-transform`, `sim-lib-music-lift`,
`sim-lib-discrete-search`, `sim-lib-pitch-dissonance`,
`sim-lib-sound-dissonance`, and `sim-lib-sound-tuning`. Those crates continue to
own exact score identity, additive staff transforms, generic bounded search,
MIDI performance semantics, contextual pitch models, psychoacoustic curves, and
tuning. This crate owns their event-window composition, inspectable reports,
and consonance-specific completion policy.
