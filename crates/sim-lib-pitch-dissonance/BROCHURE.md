# sim-lib-pitch-dissonance

In one line: Scores how tense or restful a group of notes sounds, from several theoretical points of view at once.

## What it gives you

This rates a collection of pitch classes for dissonance against a set of interchangeable models. It offers an interval-vector weighting, a Forte-style complexity measure, a key-relative model that weighs how notes function in a key, and a tritone-density ratio. It also compares contextual pitch windows with separately named roughness, commonality, leading, motion, pseudo-partial, multiplicity-aware interval-vector, and exact-ratio components. Each result keeps roughness mass, normalized density, harmonic context, and provenance separate, with explicit interval-difference, merge, context-window, duplicate, voice-identity, normalization, and weighting policy. The historical tritone-density binning is still available, but only as a named compatibility dialect. A registry runs every model at once so you can compare readings, and the whole set installs as a runtime library you can call on demand.

## Why you will be glad

- Get typed sonance components instead of an unlabeled tension scalar.
- Compare several theories of dissonance side by side.
- Weigh notes by how they function within a chosen key.
- Compare one chord or window to another without losing duplicate notes or voice identity.

## Where it fits

This is the harmonic-tension gauge of the SIM pitch family, working on the abstract pitch-class sets rather than actual sound. It complements the sound-side dissonance crate, giving composition and analysis tools a theory-based score to steer choices about harmony.
