# Analyze Derivation, All-Interval Status, and Combinatoriality

The checked scenario builds one strict row-class report and reads three kinds of
serial evidence from the same value.

The fixture row is tetrachordally derived from contiguous generator cells but
not hexachordally derived, so `generator_size` resolves to `Some(4)` while the
report still retains its smaller dyadic derivation witness. The same row also
admits prime, inversional, retrograde, and retrograde-inversional
combinatorial partners. Each witness keeps the exact operation, the successful
equal-cell partition, and complementary pitch-class masks whose union covers
the aggregate exactly.

The paired `GATE-ROW` tests add a distinct exact all-interval fixture plus
negative and malformed-request cases, so the public recipe stays focused on the
one report shape while the gate covers the edge conditions.
