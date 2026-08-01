# Validate a Five-Symbol Custom Alphabet

The Lisp surface validates a non-pitch alphabet containing `rise`, `fall`,
`hold`, `turn`, and `rest`. The exhaustive order contains each symbol once and
returns its stable alphabet id, aggregate ledger, and the Lehmer rank delegated
to `sim-lib-discrete-rank`.

Changing an order item to a foreign symbol, repeating an item, or removing an
item fails closed. The same Rust core also supports declared multiplicity,
declared omissions, projected aggregates, no-repeat subsets, and free order;
none of those policies introduces pitch, music-object, search, or enumeration
behavior into the crate.
