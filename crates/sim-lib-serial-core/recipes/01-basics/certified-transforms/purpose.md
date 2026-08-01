# Transform a Symbolic Series with Inverse Evidence

This checked Rust scenario composes retrograde with a two-position rotation
over a five-symbol gesture series. Applying the normalized transform returns a
new validated `Series` and a `TransformCertificate` containing the exact
output-to-source ordinal map, stable source and target alphabet identities,
aggregate-preservation evidence, relaxed invariants, and an inverse.

The scenario applies that inverse and recovers the source exactly. Invalid
ordinal maps and incomplete symbol maps cannot become operations: constructors
reject them before application. Exhaustive small-alphabet law tests obtain
permutations through `sim-lib-discrete-rank`; serial-core contains no search or
enumeration engine.
