# sim-lib-sound-dissonance

In one line: Estimates how rough or smooth two or more sounds are together, using established psychoacoustic models.

## What it gives you

This scores the sensory roughness of sound using a family of well-known psychoacoustic estimators -- Plomp-Levelt, Sethares, Helmholtz beating, and harmonic entropy. A registry lets you look them up by name and run them, and a runtime surface installs the whole set as a library. Unlike theory-based scoring, these models work from the actual spectral content, so they judge how a combination genuinely sounds to the ear.

## Why you will be glad

- Estimate the roughness of a sound as the ear would hear it.
- Choose among several respected psychoacoustic models.
- Look models up by name and run them through one registry.

## Where it fits

This is the sound-side tension gauge of the SIM audio family, the acoustic counterpart to the pitch-set dissonance crate. Where that one scores abstract note sets, this one scores real spectra, giving synthesis and tuning tools an ear-based measure to work with.
