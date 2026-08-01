# sim-lib-pitch-serial

`sim-lib-pitch-serial` is SIM's strict twelve-tone row-theory core. It composes
the canonical `PitchClass` identity from `sim-lib-pitch-core` with the
exhaustive exactly-once aggregate proof from `sim-lib-serial-core`; it does not
define another pitch type or another generic permutation engine.

```rust
use sim_lib_pitch_core::PitchClass;
use sim_lib_pitch_serial::{
    RowFamily, RowLabelConvention, RowOperation, ToneRow,
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
# Ok::<(), Box<dyn std::error::Error>>(())
```

P, I, R, and RI are total affine/reversal operations. Their `addend` is reduced
modulo twelve, transformed rows are constructed through a private
invariant-preserving path, and every result retains its normalized operation
identity. Labels are separate values selected by either the first/last-pitch or
operation-index convention.

See the embedded `row-family-matrix` Lisp descriptor and the `GATE-ROW`
integration tests for the Op. 25 fixture, all four families, inverse laws,
malformed aggregates, and label disagreement.
