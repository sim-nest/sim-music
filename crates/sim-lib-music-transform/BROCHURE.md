# sim-lib-music-transform

In one line: Reworks musical material with the classic moves -- transpose, invert, reverse, stretch, and compress.

## What it gives you

This applies transformations to music. It covers the classic operations a composer reaches for -- transpose, invert, retrograde, augment, and diminish -- plus configurable remaps of pitch and time, pattern mutators, and a gated custom event-filter pipeline for bespoke changes. Each transform reads a music object into a canonical piano roll and returns new music, optionally paired with diagnostics that describe what changed, so edits stay traceable rather than mysterious.

## Why you will be glad

- Transpose or invert a passage with a single operation.
- Stretch or compress timing to reshape a phrase.
- See a record of what each transform changed.

## Where it fits

This is the editing bench of the SIM music family. It takes the core music model and reshapes it, feeding altered material back to players, notation, and lowering. It gives composers and agents a dependable set of moves for developing and varying a piece.
