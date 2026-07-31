# sim-lib-sound-audio-lift

In one line: Listens to raw audio and works out the notes hiding inside it.

## What it gives you

This analyses raw recorded audio and lifts it into pitched note candidates. YIN and probabilistic YIN recover a monophonic contour with confidence, frequency bounds, voiced probability, rejected alternatives, and exact frame provenance. For polyphonic material, the existing spectral-peak and harmonic-comb candidates continue into identity-bearing partial tracks through certified assignment and DTW continuity evidence, with explicit birth, death, gap, crossing, work, and track-limit policy. Phase-preserving STFT and checked overlap-add reconstruction make those frames reusable, while tuning-anchored constant-Q and explicit chroma folding turn them into octave-independent musical evidence. With its music option turned on, results convert straight into piano rolls, diff rolls, and counterpoint you can work with.

## Why you will be glad

- Pull note candidates out of a plain audio recording.
- Follow vibrato with YIN/pYIN or separate crossing partials under declared bounds.
- Review uncertainty, rejected hypotheses, source frames, and algorithm receipts.
- Round-trip framed audio only after the declared analysis/synthesis windows pass COLA.
- Build bounded constant-Q and chroma frames that retain tuning, weighting, and fold policy.
- Choose the analysis approach that suits your material.
- Turn the result straight into a workable piano roll.

## Where it fits

This is the ears of the SIM sound stack, the bridge from audio back up to notes. It complements the MIDI-side lifting crate, letting the constellation take real sound as input and hand structured music to the analysis and notation tools downstream.
