# sim-lib-music-counterpoint

Exact counterpoint rule reports, graph-backed stretto analysis, and bounded
constraint generation.

The crate inspects existing counterpoint without rewriting it. Species and open
rule sets are data, every violation carries voices, notes, an exact span, a rule,
and metric evidence, and stretto candidates remain explicitly separate from
generation. The generator compiles the same rules to finite pitch variables and
domains for `sim-lib-discrete-search`, retains its work/frontier/result,
cancellation, seed, and partial-result receipt, and returns only analyzer-legal
alternatives. Generated voices are existing content-bound `ConsonancePatch`
additions whose inverse restores the fixed cantus exactly.
