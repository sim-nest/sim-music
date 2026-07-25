# sim-lib-pitch-ratio

In one line: Keeps just-intonation style intervals as exact reduced ratios.

## What it gives you

It validates positive ratios, reduces them to canonical identity, optionally
folds them into one octave, exposes bounded prime-factor vectors, ranks finite
prime-limit vectors with the discrete mixed-radix machinery, and searches for
nearby ratios under explicit `SearchControl` bounds.

## Why you will be glad

- Compare intervals by exact rational identity, not rounded cents.
- Constrain ratio work with octave and prime-limit policy.
- Carry search receipts that prove work bounds and approximation error.

## Where it fits

This is the exact interval layer of the SIM pitch family. It composes the
discrete rank/search crates and the existing rational-number substrate while
leaving pitch core and tuning focused on their current jobs.
