# Serial Core

Build serial technique over the symbols the work actually uses.

`sim-lib-serial-core` validates and transforms ordered series over any finite
alphabet, from pitch classes to gestures, durations, dynamics, registers, or a
project's own symbol vocabulary. Aggregate intent remains explicit data:
exactly-once, no-repeat, declared multiplicity, declared omissions, projected
classes, or free order. Every accepted series carries an `AggregateLedger`
showing what was observed and expected.

Series retain symbols rather than user ordinals. Stable alphabet identities,
duplicate rejection, foreign-symbol checks, and impossible-rule checks keep the
boundary honest. Retrograde, rotation, block partitions, ordinal permutations,
cyclic relabeling, and caller-defined alphabet bijections validate their maps
before application and return `TransformCertificate` evidence: exact order map,
source and target alphabet, aggregate preservation, relaxed invariants, and an
inverse when the algebra supplies one.

Composition normalizes to deterministic finite maps. When an accepted order is
a permutation, its rank comes from SIM's shared discrete rank owner; this crate
never grows a private search or enumeration engine.

This is the open foundation for twelve-tone rows without making twelve pitches
the definition of serial music.
