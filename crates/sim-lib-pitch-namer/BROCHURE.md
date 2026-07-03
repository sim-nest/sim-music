# sim-lib-pitch-namer

In one line: Names a group of notes in every musical language at once -- and translates a name from one school to another.

## What it gives you

This is the aggregator over the pitch-naming schools. It defines the shared naming interface and the taxonomy of schools, then drives every built-in one -- Forte set-class names, functional roman numerals, plain prime forms, neo-Riemannian labels, and jazz chord symbols -- through a single registry. That registry can label a set of notes in every school at once and translate a label from one school into another, so a chord can carry many names side by side.

## Why you will be glad

- Get every school's name for a chord in one call.
- Translate a label between roman numerals, jazz, and set theory.
- Add or compare naming schools through one shared registry.

## Where it fits

This is the naming hub of the SIM pitch family. The individual schools live in sibling crates; this one composes them, resolves conflicts of vocabulary, and exposes the whole set as a runtime library so tools can speak whichever harmonic language a user prefers.
