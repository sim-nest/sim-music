# sim-lib-sound-render

In one line: Renders sound, transforms time and pitch, and reports loudness, true peak, and normalization decisions.

## What it gives you

This renders tones into playable audio, then handles the policy-heavy offline work around that PCM. It can stretch duration and shift pitch independently through the existing phase-preserving STFT, measure EBU R128 momentary and integrated loudness with ITU-R BS.1770 K-weighting and true peak, and normalize through one fully reported gain. Its float output is never silently clipped or limited.

## Why you will be glad

- Bounce synthesized tones straight to a WAV file.
- Mix scheduled tones with their own timing and panning.
- Stretch or pitch audio with explicit phase-lock, transient, unwrap, and frequency policy.
- Review gates, gain, dBTP ceilings, and clipping instead of trusting a hidden mastering step.

## Where it fits

This is the offline output and measurement stage of the SIM sound family. It composes transform math from numbers-signal and STFT framing from audio-lift. Final channel mapping, PCM16 quantization, seeded dither, and canonical WAV interchange stay with stream-file.
