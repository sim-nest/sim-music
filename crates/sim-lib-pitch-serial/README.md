# sim-lib-pitch-serial

`sim-lib-pitch-serial` is SIM's strict twelve-tone row-theory core. It composes
the canonical `PitchClass` identity from `sim-lib-pitch-core` with the
exhaustive exactly-once aggregate proof from `sim-lib-serial-core`; it does not
define another pitch type or another generic permutation engine. It also owns
validated row partitions, partition similarity/interlocking reports, partition
mosaics, chord-free vertical collections built directly from strict row
ordinals, validated row rotations and ordinal permutations, affine pitch-class
maps, and block-product pitch reservoirs with explicit provenance.

```rust
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    AffinePitchMap, BlockOrder, PitchTransformOutput, RowFamily, RowFamilySet,
    RowLabelConvention, RowMatrix, RowOperation, ToneRow, try_partition,
    verticalize,
};

let row = ToneRow::try_from_classes([
    PitchClass::E, PitchClass::F, PitchClass::G, PitchClass::CS,
    PitchClass::FS, PitchClass::DS, PitchClass::GS, PitchClass::D,
    PitchClass::B, PitchClass::C, PitchClass::A, PitchClass::AS,
])?;
let form = row.apply(RowOperation::new(RowFamily::P, 0));

assert_eq!(form.operation().to_string(), "P0");
assert_eq!(
    form.label(RowLabelConvention::FirstLastPitch).to_string(),
    "P4",
);
assert_eq!(
    form.label(RowLabelConvention::OperationIndex).to_string(),
    "P0",
);
let family = RowFamilySet::of(&row);
assert_eq!(family.aliases().len(), 48);
assert!(family.distinct_forms().len() <= 48);

let report = sim_lib_pitch_serial::analyze_row_class(&row);
assert!(report
    .combinatoriality
    .iter()
    .all(|partner| partner.source.union(partner.complement).count_bits() == 12));

let matrix = RowMatrix::new(&row, RowLabelConvention::FirstLastPitch);
assert_eq!(matrix.source(), &row);
assert_eq!(matrix.render_data().cells().len(), 144);

let partition = try_partition(
    vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]],
    BlockOrder::total(),
)?;
let vertical = verticalize(&row, &partition);
assert_eq!(vertical.slices.len(), 3);
assert!(vertical.aggregate_coverage.complete);

let affine = AffinePitchMap::new(5, 3).apply(&row);
assert!(matches!(affine, PitchTransformOutput::Row(_)));
# Ok::<(), Box<dyn std::error::Error>>(())
```

P, I, R, and RI are total affine/reversal operations. Their `addend` is reduced
modulo twelve, transformed rows are constructed through a private
invariant-preserving path, and every result retains its normalized operation
identity. Labels are separate values selected by either the first/last-pitch or
operation-index convention. `RowFamilySet` preserves all 48 operation aliases
while deduplicating equal row values caused by symmetry. `analyze_row_class`
adds generator-cell derivation, exact all-interval evidence, stabilizers, form
equivalences, and combinatorial partner witnesses without introducing a second
permutation engine. `ToneRow::rotate` and `ToneRow::try_permute_ordinals`
reuse validated ordinal bijections from `sim-lib-serial-core` rather than
spelling a second permutation validator. `AffinePitchMap` returns a strict row
only for the bijective `M1`, `M5`, `M7`, and `M11` multipliers; non-bijective
maps return an ordered `PitchReservoir` that reports relaxed row invariants.
`try_partition` validates nonempty disjoint ordinal blocks that cover the row
exactly once while preserving whether order is total, partial, or intentionally
absent within and between blocks. The partition reports lift those ordinals
into similarity, interlocking, mosaic, aggregate coverage, and verticalized set
data without inventing a chord type, and `multiply_partitions` projects one
block's interval content onto another while retaining per-block provenance in
the resulting reservoir. `RowMatrix` retains its source, convention,
coordinate-bearing cells, P/I line operations, and P/R/I/RI edge labels; both
its structured data and ASCII display project the same object.

See the embedded `row-family-matrix` and `row-class-analysis` Rust scenarios and
the `GATE-ROW` integration tests for the Op. 25 fixture, derived rows,
all-interval rows, all 48 aliases, symmetric-family deduplication, every matrix
row and column, inverse laws, validated ordinal permutations, `M1/M5/M7/M11`
affine bijections, non-bijective reservoirs, block-product provenance,
combinatorial partitions, and label disagreement.
