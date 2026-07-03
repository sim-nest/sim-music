# sim-lib-pitch-namer-forte

In one line: Gives any group of notes its Forte set-class name, the standard label set theory uses.

## What it gives you

This implements the Forte naming school. It maps a group of pitch classes to its Forte set-class name -- for instance 4-27 for a dominant seventh -- through a lookup table of prime-form patterns. Before matching, it normalises the set to prime form, so any transposition or rotation of the same shape resolves to one name. The result is a stable, standard label that scholars and analysis tools recognise.

## Why you will be glad

- Label any chord with its recognised Forte set-class name.
- Get the same name regardless of how the set is transposed.
- Lean on a settled lookup rather than hand-classifying sets.

## Where it fits

This is one voice in the SIM pitch-naming chorus, the set-theory one. It plugs into the naming aggregator alongside the jazz, roman, and Riemannian schools, contributing the Forte vocabulary that formal analysis relies on.
