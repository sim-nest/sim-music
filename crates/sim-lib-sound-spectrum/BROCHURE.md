# sim-lib-sound-spectrum

In one line: Breaks a sound into its frequencies and measures its character -- brightness, peaks, and how it changes.

## What it gives you

This gives you the frequency-domain view of a sound. It builds a spectrum -- a magnitude picture across frequency -- either from a synthesized tone or from recorded samples, then measures the common descriptors that summarise timbre: the peaks, the spectral centroid that tracks brightness, flatness, rolloff, and flux that tracks change over time. It turns a raw sound into a handful of readable numbers about its character.

Recorded-sample transforms reuse the generic numbers-signal real FFT. This
sound crate keeps the public `Spectrum` API, physical hertz bins, amplitude
descriptors, and source provenance without maintaining a second Fourier
implementation or convention.

## Why you will be glad

- See which frequencies make up a sound.
- Measure brightness and other timbre traits as plain numbers.
- Analyse either synthesized tones or recorded audio.
- Share one checked transform implementation with non-audio signal work.

## Where it fits

This is the spectral-analysis tool of the SIM sound family. It feeds descriptors to the dissonance, timbre, and audio-lifting crates, giving the constellation a shared way to describe and compare the character of a sound.
