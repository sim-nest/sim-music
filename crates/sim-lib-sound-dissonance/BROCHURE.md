# sim-lib-sound-dissonance

In one line: Estimates how rough or smooth two or more sounds are together, using established psychoacoustic models.

## What it gives you

This scores the sensory roughness of sound using a family of well-known psychoacoustic estimators -- Plomp-Levelt, Sethares, Helmholtz beating, and harmonic entropy. Results keep roughness mass, normalized density, harmonic context, curve family, and partial-policy evidence separate, including skipped inaudible pairs. A registry lets you look models up by name and run them, and a runtime surface installs the whole set as a library. Unlike theory-based scoring, these models work from the actual spectral content, so they judge how a combination genuinely sounds to the ear.

## Why you will be glad

- Estimate roughness with checked finite inputs and explicit partial-pair evidence.
- Choose among several respected psychoacoustic models.
- Look models up by name and run them through one registry.

## Where it fits

This is the sound-side tension gauge of the SIM audio family, the acoustic counterpart to the pitch-set dissonance crate. Where that one scores abstract note sets, this one scores real spectra, giving synthesis and tuning tools an ear-based measure to work with.
