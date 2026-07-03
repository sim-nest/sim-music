# sim-lib-pitch-dissonance

In one line: Scores how tense or restful a group of notes sounds, from several theoretical points of view at once.

## What it gives you

This rates a collection of pitch classes for dissonance against a set of interchangeable models. It offers an interval-vector weighting, a Forte-style complexity measure, a key-relative model that weighs how notes function in a key, and a tritone-density ratio. A registry runs every model at once so you can compare readings, and the whole set installs as a runtime library you can call on demand.

## Why you will be glad

- Get a numeric read on how tense a chord sounds.
- Compare several theories of dissonance side by side.
- Weigh notes by how they function within a chosen key.

## Where it fits

This is the harmonic-tension gauge of the SIM pitch family, working on the abstract pitch-class sets rather than actual sound. It complements the sound-side dissonance crate, giving composition and analysis tools a theory-based score to steer choices about harmony.
