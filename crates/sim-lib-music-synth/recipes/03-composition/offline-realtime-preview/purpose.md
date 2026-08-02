# Offline timbre render and realtime DSP preview

This composition, paired with a checked conformance test, maps the existing
synthesis catalog to its current owners instead of copying signal code.
`sim-lib-sound-timbre` owns additive
partials, filtered timbres, Karplus-Strong, and compact FM pairs;
`sim-lib-music-synth` owns oscillators, envelopes, subtractive voices, DX7 FM,
and modeled modular instruments; and `sim-lib-audio-dsp` owns waveshaping,
filters, dynamics, delays, and modulation effects.

The offline branch renders the FM-bell timbre with `sim-lib-sound-render` into a
deterministic f32 PCM buffer. The realtime branch sends the same declared source
through the audio graph's filter and limiter at `default-audio`; the live runner
prepares bounded buffers and queues before entering the callback, whose checked
contract permits no allocation growth, locks, or I/O. Buffered preview packaging
happens outside that callback boundary.

Inputs and artifacts are Table/Dir values addressed by explicit handles. Every
output sets `replace` to false, so rerunning the recipe cannot overwrite an
existing artifact. Canonical bounded PCM16 WAV byte/stream input and output stay
with `sim-lib-stream-file`; converting the offline f32 buffer to PCM16 and
choosing dither policy are deliberately recorded for `MUSICALGOS4.43`.
