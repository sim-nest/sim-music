# sim-lib-music-counterpoint

In one line: It explains contrapuntal mistakes exactly and maps where a theme can
overlap with transformed copies of itself.

## What it gives you

This library aligns every voice at exact rational note boundaries and checks
inspectable species or caller-authored rules for intervals, motion, range,
crossing, overlap, duration, and prepared or resolved dissonance. Each failure
names the involved voices and notes, the exact time span, the rule, and the
measurement that failed.

Its stretto side derives bounded delayed and transformed entries, records legal
pairs in the shared graph model, finds pairwise-compatible cliques and chains,
and offers fused counterpoint views with transform provenance. These are
analysis candidates, never silently presented as generated composition.

## Why you will be glad

- Diagnose a passage without losing exact timing or source identity.
- Edit species and open policies as ordinary data.
- Inspect compatible stretto couples, cliques, components, and chains.
- Trace every derived entry back to an existing transform operation.

## Where it fits

It composes music-core, exact consonance windows, music transforms, and discrete
graphs. A later bounded generator can consume its rules and candidate evidence
without embedding search inside this analysis crate.
