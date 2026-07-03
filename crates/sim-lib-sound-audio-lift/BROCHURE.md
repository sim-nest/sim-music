# sim-lib-sound-audio-lift

In one line: Listens to raw audio and works out the notes hiding inside it.

## What it gives you

This analyses raw recorded audio and lifts it into pitched note candidates. Its analysers -- one tracking spectral peaks, one combing for harmonics -- break the sound into per-window frames and assemble candidate notes under configurable options, so you get a considered guess at what was played rather than just a waveform. With its music option turned on, the results convert straight into piano rolls, diff rolls, and counterpoint you can work with.

## Why you will be glad

- Pull note candidates out of a plain audio recording.
- Choose the analysis approach that suits your material.
- Turn the result straight into a workable piano roll.

## Where it fits

This is the ears of the SIM sound stack, the bridge from audio back up to notes. It complements the MIDI-side lifting crate, letting the constellation take real sound as input and hand structured music to the analysis and notation tools downstream.
