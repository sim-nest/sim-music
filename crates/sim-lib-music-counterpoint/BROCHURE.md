# sim-lib-music-counterpoint

In one line: Explains contrapuntal mistakes, maps thematic overlap, and generates bounded reversible companion voices.

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

Its generation side compiles the same rule set to inspectable finite CSP
variables and pitch domains, then delegates propagation, scoring, deterministic
seed order, work/frontier/result limits, and cancellation to the shared bounded
search engine. Each legal result is a strictly additive, content-bound patch
whose checked inverse restores the fixed cantus byte-for-byte.

## Why you will be glad

- Diagnose a passage without losing exact timing or source identity.
- Edit species and open policies as ordinary data.
- Inspect compatible stretto couples, cliques, components, and chains.
- Trace every derived entry back to an existing transform operation.
- Ask for one or more generated voices without accepting an unbounded solver.
- Compare alternatives with honest search and diversity receipts.

## Where it fits

It composes music-core, exact consonance windows and reversible patches, music
transforms, discrete graphs, and the generic discrete-search engine.
