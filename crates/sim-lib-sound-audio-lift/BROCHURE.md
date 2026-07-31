# sim-lib-sound-audio-lift

In one line: Listens to raw audio and works out the notes hiding inside it.

## What it gives you

This analyses raw recorded audio and lifts it into pitched note candidates and reviewable musical features. The loadable `sound/lift/pitch-track` call and matching Rust API run YIN and probabilistic YIN to recover a monophonic contour with confidence, frequency bounds, voiced probability, rejected alternatives, explicit interpolation policy, and exact frame provenance. The composed `sound/lift/analyze` call adds bounded onset strength and latency-aware peak picking, certified varying-tempo beat paths, meter hypotheses, zero-crossing rate, mel/Bark/ERB filterbanks, policy-complete MFCCs, chroma, and key/chord sequences with posterior alternatives. For polyphonic material, the existing spectral-peak and harmonic-comb candidates continue into identity-bearing partial tracks through certified assignment and DTW continuity evidence, with explicit birth, death, gap, crossing, work, and track-limit policy. Phase-preserving STFT and checked overlap-add reconstruction make those frames reusable, while tuning-anchored constant-Q and explicit chroma folding turn them into octave-independent musical evidence. With its music option turned on, note results convert straight into piano rolls, diff rolls, and counterpoint you can work with.

## Why you will be glad

- Pull note candidates out of a plain audio recording.
- Follow vibrato with YIN/pYIN or separate crossing partials under declared bounds.
- Review uncertainty, rejected hypotheses, source frames, and algorithm receipts.
- Detect attacks under explicit lookahead/minimum-distance policy and retain rejected peaks.
- Follow changing tempo through a shared certified graph DP and compare meter hypotheses.
- Compute full-rate mel, Bark, or ERB MFCCs with explicit log, DCT, lifter, and normalization policy.
- Decode keys and chords through the shared finite HMM while retaining confidence and alternatives.
- Round-trip framed audio only after the declared analysis/synthesis windows pass COLA.
- Build bounded constant-Q and chroma frames that retain tuning, weighting, and fold policy.
- Choose the analysis approach that suits your material.
- Turn the result straight into a workable piano roll.

## Where it fits

This is the ears of the SIM sound stack, the bridge from audio back up to notes. It complements the MIDI-side lifting crate, letting the constellation take real sound as input and hand structured music to the analysis and notation tools downstream.
