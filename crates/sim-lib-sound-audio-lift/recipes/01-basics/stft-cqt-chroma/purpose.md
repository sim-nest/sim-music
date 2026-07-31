# Reconstructable STFT and tuning-anchored chroma (descriptor)

The paired Rust specimen uses periodic Hann analysis and synthesis windows with a 128-sample frame, 32-sample hop, centered zero padding, negative-forward phase, and forward normalization. It checks the resulting 1.5 constant-overlap-add gain before inverse reconstruction and measures the round trip.

The same specimen runs a bounded 12-bin-per-octave constant-Q grid from 220 Hz through 880 Hz against the equal-temperament A4 = 440 Hz reference, weights bins by power, folds octaves with the named `Sum` policy, and applies per-frame L1 normalization. The output retains the tuning, reference, weighting, fold, work, and realized-kernel facts needed to reproduce the profile.
