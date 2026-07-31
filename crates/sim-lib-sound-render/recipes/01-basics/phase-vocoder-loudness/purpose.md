# Offline phase-vocoder and loudness evidence

Compose `phase_vocode` with the existing phase-preserving STFT and generic
numbers-signal FFT/phase-unwrapping owners. Retain stretch, pitch, phase-lock,
transient-reset, unwrap, instantaneous-frequency, and output-work policy in the
result.

Use `measure_loudness` for 400 ms momentary blocks, EBU R128 absolute/relative
gating, ITU-R BS.1770 K-weighting, and bandlimited true peak. Use
`normalize_loudness` only when its report can retain requested and applied gain,
gain-bound decisions, true-peak ceiling violations, and unclipped float samples.
PCM16 channel mapping, quantization, and dither remain in `sim-lib-stream-file`.
