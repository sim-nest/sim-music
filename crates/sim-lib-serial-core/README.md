# sim-lib-serial-core

`sim-lib-serial-core` is SIM's pitch-independent finite serial calculus. It
stores alphabet symbols in ordered series, validates explicit aggregate rules,
retains count evidence, and delegates permutation rank to `sim-discrete`.

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
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the embedded `custom-alphabet` cookbook recipe for the matching Lisp
surface in `sim-lib-music-shapes`.
