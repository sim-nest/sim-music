# Compare Row Operations and Labels

The fixture is Schoenberg's documented Op. 25 row in canonical numeric pitch
classes: E, F, G, C-sharp, F-sharp, E-flat, A-flat, D, B, C, A, B-flat, or
`[4,5,7,1,6,3,8,2,11,0,9,10]`.

The four zero-addend operations retain the algebraic identities `P0`, `I0`,
`R0`, and `RI0`. Because this source begins on pitch class 4, the explicit
first/last-pitch convention prints `P4`, `I8`, `R4`, and `RI8`; the
operation-index convention prints the original zero indices. The paired
`GATE-ROW` Rust specimen checks these values, all four form equations, inverse
laws for every modulo-twelve addend, and malformed aggregate refusal.

Load `PitchShapesLib` to use the canonical comma-separated `pitch/ToneRow`
citizen adapter. The adapter round-trips the strict row value; it does not move
operation provenance or label policy into the codec.
