# sim-lib-pitch-serial

`sim-lib-pitch-serial` is SIM's strict twelve-tone row-theory core. It composes
the canonical `PitchClass` identity from `sim-lib-pitch-core` with the
exhaustive exactly-once aggregate proof from `sim-lib-serial-core`; it does not
define another pitch type or another generic permutation engine.

```rust
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    RowFamily, RowFamilySet, RowLabelConvention, RowMatrix, RowOperation,
    ToneRow,
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

let matrix = RowMatrix::new(&row, RowLabelConvention::FirstLastPitch);
assert_eq!(matrix.source(), &row);
assert_eq!(matrix.render_data().cells().len(), 144);
# Ok::<(), Box<dyn std::error::Error>>(())
```

P, I, R, and RI are total affine/reversal operations. Their `addend` is reduced
modulo twelve, transformed rows are constructed through a private
invariant-preserving path, and every result retains its normalized operation
identity. Labels are separate values selected by either the first/last-pitch or
operation-index convention. `RowFamilySet` preserves all 48 operation aliases
while deduplicating equal row values caused by symmetry. `RowMatrix` retains its
source, convention, coordinate-bearing cells, P/I line operations, and P/R/I/RI
edge labels; both its structured data and ASCII display project the same object.

See the embedded `row-family-matrix` Rust scenario and the `GATE-ROW`
integration tests for the Op. 25 fixture, all 48 aliases, symmetric-family
deduplication, every matrix row and column, inverse laws, malformed aggregates,
and label disagreement.
