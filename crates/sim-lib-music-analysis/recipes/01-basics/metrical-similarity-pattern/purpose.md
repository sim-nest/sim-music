# Exact Quantization, Similarity, and Pattern Evidence (descriptor)

The music adapter keeps policy and identity visible while delegating algorithms:

- `quantize_staff` globally aligns distinct rational onsets through
  `sim-lib-discrete-graph` DTW. The lattice declares quarter-note tempo, meter,
  primary subdivision, exact swing ratio, tuplet divisions, and tolerance.
  Output time remains rational. Every movement names the original event and
  exact before/after times; events outside tolerance are preserved.
- `compare_sequences` names its pitch or rhythm extractor and explicitly states
  transposition and uniform-time-scale invariance. Every ranked transform keeps
  the generic DTW certificate/receipt and `sim-lib-numbers-signal` normalized
  cross-correlation result, lag, coefficient, and combined cost.
- `discover_patterns` admits only a bounded number of windows, hash bytes, and
  candidate pairs. Hashes are filters, never proof: same-hash pairs are compared
  as exact rational canonical forms through `sim-lib-discrete-search`. The report
  retains the unmodified `SearchReceipt`, minimum support, overlap behavior,
  stable occurrence/event identities, exact spans, affine transforms, and costs.

No operation mutates its borrowed source, rounds an exact time, trusts a hash
collision, or silently enables an invariance.
