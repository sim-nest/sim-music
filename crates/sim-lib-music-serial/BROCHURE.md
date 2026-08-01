# sim-lib-music-serial

In one line: keeps serial plans immutable, explicit, and honest through realization.

This library is the structural source for serial practice. A plan retains exact
row identity, stable event identity, role/origin provenance, parent evidence,
voice identity, simultaneous groups for chords, and a validated precedence DAG
for what must happen before what. It never fabricates a total onset order just
to make a chord easy to store.

Use it when one row needs to span several voices, when a single structural
ordinal participates in a vertical sonority, or when ornaments and derived
material must stay visibly distinct from the row statement they depend on. The
plan validates row and event references, structural coverage, parent evidence,
and cycles up front. A SerialPractice then evaluates a named reading
(`StructuralPlan`, `DeclaredRoles`, or `AllSounding`) through open
`PracticeRule` components and returns an inspectable invariant ledger rather
than a hidden strict/loose switch. Built-in rules cover aggregate, order,
repeats, doublings, simultaneity, row mixing, foreign material, and parameter
exhaustion, and any relaxation must name an explicit waiver. Strict
realization then adds explicit register, duration, velocity, articulation, rest,
tie, and simultaneity choices while preserving the source plan and ordinal
provenance through canonical music-core staff, piano-roll, and score rendering.
