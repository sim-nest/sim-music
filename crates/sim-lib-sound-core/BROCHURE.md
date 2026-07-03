# sim-lib-sound-core

In one line: The basic building blocks of sound -- frequency, loudness, and the ingredients of a tone.

## What it gives you

This defines the foundation acoustic types the whole synthesis layer shares: frequency, amplitude, and phase quantities, and the partial, envelope, and tone models that build a spectral tone out of sinusoidal components. It also includes helpers for sensible default envelopes and for converting equal-temperament pitches to frequencies. These are the small, shared pieces every sound crate assembles into something you can hear.

## Why you will be glad

- Describe a tone as its partials, envelope, and pitch.
- Convert a note to its frequency with a ready helper.
- Share one set of sound quantities across every audio crate.

## Where it fits

This is the bedrock of the SIM sound family. Spectrum, timbre, tuning, rendering, and the bridge all build on these primitives, so every acoustic tool in the constellation describes frequency, loudness, and tone the same way.
