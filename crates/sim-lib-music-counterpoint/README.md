# sim-lib-music-counterpoint

Exact counterpoint rule reports and graph-backed stretto analysis.

The crate inspects existing counterpoint without rewriting it. Species and open
rule sets are data, every violation carries voices, notes, an exact span, a rule,
and metric evidence, and stretto candidates remain explicitly separate from
generation. Compatibility uses the shared discrete graph type; materialized
retrograde, inversion, transposition, and duration candidates use the existing
music transform owner.
