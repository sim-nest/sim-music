# sim-lib-serial-core

In one line: builds serial technique over the symbols the work actually uses.

## What it gives you

`sim-lib-serial-core` validates and transforms ordered series over any finite
alphabet, from pitch classes to gestures, durations, dynamics, registers, or a
project's own symbol vocabulary. Aggregate intent remains explicit data:
exactly-once, no-repeat, declared multiplicity, declared omissions, projected
classes, or free order. Every accepted series carries an `AggregateLedger`
showing what was observed and expected.

## Why you will be glad

- Keep serial validation generic instead of hard-wiring pitch into the substrate.
- Get certified transforms with exact order maps, preserved aggregates, and inverses when the algebra supplies them.
- Reuse SIM's shared rank/search owners instead of growing a private enumeration engine.

## Where it fits

This is the open foundation under twelve-tone rows, integral parameter cycles,
and any other finite serial vocabulary. Series retain symbols rather than user
ordinals, and retrograde, rotation, partitions, permutations, cyclic
relabeling, and caller-defined bijections all validate their maps before
application.
