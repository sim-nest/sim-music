# sim-lib-serial-core

`sim-lib-serial-core` is SIM's pitch-independent finite serial calculus. It
stores alphabet symbols in ordered series, validates explicit aggregate rules,
retains count evidence, supplies total certified transforms, and delegates
permutation rank to `sim-discrete`.

The crate deliberately has no pitch, score, runtime, search, or enumeration
dependency. A five-symbol gesture alphabet is as native as a later chromatic
pitch-class adapter.

```rust
use sim_lib_serial_core::{AggregateRule, AlphabetId, FiniteAlphabet, Series};

let alphabet = FiniteAlphabet::try_new(
    AlphabetId::try_new("gesture/five-v1")?,
    vec!["rise", "fall", "hold", "turn", "rest"],
)?;
let series = Series::try_new(
    alphabet,
    AggregateRule::exhaustive_exactly_once(),
    vec!["turn", "rise", "rest", "fall", "hold"],
)?;
assert!(series.ledger().is_exhaustive_exactly_once());

let transformed = series.apply(&sim_lib_serial_core::SeriesTransform::retrograde(5))?;
let restored = transformed.series.apply(
    transformed.certificate.inverse.as_ref().expect("retrograde has an inverse"),
)?;
assert_eq!(restored.series, series);
assert!(transformed.certificate.aggregate_preserved);
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the embedded `custom-alphabet` cookbook recipe for the matching Lisp
validation surface in `sim-lib-music-shapes`, and `certified-transforms` for a
checked Rust composition and inverse law.
