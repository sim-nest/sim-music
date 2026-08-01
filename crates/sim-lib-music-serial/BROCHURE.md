# sim-lib-music-serial

In one line: keeps serial plans immutable, explicit, and honest before they are realized.

This library is the structural source for serial practice. A plan retains exact
row identity, stable event identity, role/origin provenance, parent evidence,
voice identity, simultaneous groups for chords, and a validated precedence DAG
for what must happen before what. It never fabricates a total onset order just
to make a chord easy to store.

Use it when one row needs to span several voices, when a single structural
ordinal participates in a vertical sonority, or when ornaments and derived
material must stay visibly distinct from the row statement they depend on. The
plan validates row and event references, structural coverage, parent evidence,
and cycles up front, so later MIDI, audio, notation, or completion layers can
consume a trustworthy immutable source instead of repairing ambiguity after the
fact.
