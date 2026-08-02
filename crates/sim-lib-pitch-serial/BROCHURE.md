# sim-lib-pitch-serial

In one line: turns one exact chromatic aggregate into trustworthy P, I, R, and RI families without losing provenance.

## What it gives you

`sim-lib-pitch-serial` accepts only rows containing each of SIM's canonical
twelve `PitchClass` values exactly once. It reuses the general serial aggregate
validator, so malformed rows fail with typed evidence while pitch identity
stays shared with the rest of the music stack.

## Why you will be glad

- Keep every row operation total, normalized, and explicit about the form that produced it.
- Preserve all 48 aliases even when symmetry collapses them to fewer distinct rows.
- Inspect matrices, partitions, mosaics, and interlocking evidence from structured data instead of ad hoc text.

## Where it fits

Use this when twelve-tone work needs row forms, label conventions, matrices, or
partition analysis that can be trusted downstream by realization, notation, and
practice layers. The documented Schoenberg Op. 25 row starts on E, so its
identity operation is `P0` in affine algebra and `P4` under first-pitch
labeling; this crate keeps both facts instead of silently choosing one.
