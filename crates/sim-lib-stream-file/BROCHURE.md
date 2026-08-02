# sim-lib-stream-file

In one line: Owns accountable stream files plus bounded, auditable channel mapping and PCM16 conversion.

## What it gives you

This provides source and sink adapters backed by the filesystem, so a stream can be fed from a file or written out to one. Every read and write stays behind an explicit file capability and is recorded as a filesystem effect. Its WAV owner also maps finite channel layouts and quantizes float PCM with explicit clipping evidence and reproducible TPDF or first-order noise-shaped dither.

## Why you will be glad

- Play a stream out of a file or capture one into a file.
- Keep every disk access permission-gated and recorded.
- Handle both MIDI files and WAV audio through one adapter.
- Keep channel gains, quantization error, clipping, and dither seed visible in one report.

## Where it fits

This is the disk and PCM16 interchange end of the SIM streaming layer. Renderers and analyzers hand float PCM to this owner instead of growing their own channel maps, quantizers, dither loops, or WAV encoders.
