# Build a Complete Row Family and Matrix

The fixture is Schoenberg's documented Op. 25 row in canonical numeric pitch
classes: E, F, G, C-sharp, F-sharp, E-flat, A-flat, D, B, C, A, B-flat, or
`[4,5,7,1,6,3,8,2,11,0,9,10]`.

The checked Rust scenario builds all 48 P/I/R/RI aliases and verifies each form
against the operation that produced it. `RowFamilySet` also deduplicates equal
row values without discarding any alias, so symmetric rows remain honestly
addressable through every operation.

It then builds one `RowMatrix` under the explicit first/last-pitch convention.
The object retains the source row, P row operations, I column operations, all
four edge-label collections, and a coordinate on every structured cell. ASCII
is rendered from that structured projection. The paired `GATE-ROW` tests prove
every row, column, reverse reading, label edge, and coordinate against the
underlying operation rather than treating display text as the oracle.
