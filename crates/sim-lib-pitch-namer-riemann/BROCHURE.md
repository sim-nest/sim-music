# sim-lib-pitch-namer-riemann

In one line: Labels a triad by its function -- major or minor tonic -- in the neo-Riemannian style.

## What it gives you

This implements the Riemannian naming school. It labels a triad by its functional quality relative to its root, telling a major triad from a minor one through case -- an upper-case letter for the major function, a lower-case letter for the minor. It rotates a group of notes so the root sits at zero, matches the major or minor triad pattern, and returns nothing for sets that are not triads, keeping its answers honest.

## Why you will be glad

- Tell major from minor triads with a clear functional label.
- Get a root-relative reading rather than a fixed spelling.
- See a clean no-match when a set is not a triad.

## Where it fits

This is the neo-Riemannian voice in the SIM pitch-naming family. It contributes function-based triad labels to the naming aggregator, sitting beside the Forte, jazz, and roman schools so harmony can be described in transformational terms.
