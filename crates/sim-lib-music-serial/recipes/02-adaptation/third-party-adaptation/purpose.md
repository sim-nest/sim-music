Build one external-style consumer around the public serial surfaces only. The
scenario defines a seven-symbol non-pitch alphabet, validates its series from
Rust and through the Lisp `music/serial/validate` surface, then composes a
caller-owned practice rule and a caller-owned adaptive realizer without adding
an enum arm, registry singleton, or kernel change inside the production
crates.

The same specimen also proves failure stays closed. An unknown realizer id is
returned unchanged through `StrictRealizationError::UnknownRealizer`, and the
`music/SerialSeries` Shape reports `shape-serial-series` diagnostics when the
Lisp source names a symbol outside the registered seven-symbol vocabulary.
