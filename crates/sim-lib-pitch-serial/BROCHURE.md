# Twelve-Tone Row Theory

Turn one exact chromatic aggregate into trustworthy P, I, R, and RI forms.

`sim-lib-pitch-serial` accepts only rows containing each of SIM's canonical
twelve `PitchClass` values exactly once. It reuses the general serial aggregate
validator, so malformed rows fail with typed evidence while pitch identity
stays shared with the rest of the music stack.

Every operation is total and normalized modulo twelve. The returned `RowForm`
retains the operation that produced it, making later analysis and provenance
unambiguous. Printed names are deliberately separate: choose traditional
first/last-pitch labels or affine operation-index labels, and retain honest
disagreement between them.

The documented Schoenberg Op. 25 row starts on E. Its identity operation is
therefore `P0` as affine algebra and `P4` under first-pitch labeling. SIM keeps
both facts instead of silently choosing one.
