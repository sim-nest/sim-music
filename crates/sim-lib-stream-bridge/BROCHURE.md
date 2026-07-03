# sim-lib-stream-bridge

In one line: Converts flowing streams between MIDI and audio so one kind of stream can feed the other.

## What it gives you

This adapts finite streams of data packets between MIDI and PCM audio, leaning on the existing sound and audio-lifting crates to do the actual conversion. It lets a MIDI stream become an audio stream, or audio become notes, as a step in a pipeline. It deliberately does not talk to host audio or MIDI devices itself; it only reshapes the data as it passes through.

## Why you will be glad

- Feed a MIDI stream into an audio pipeline, or the reverse.
- Reuse the proven sound and lifting crates for the conversion.
- Keep device access out of it, so the piece stays predictable.

## Where it fits

This is a converter within the SIM streaming layer. It joins the MIDI and audio sides of the stream system so material can flow across that divide, sitting between the device-facing stream adapters without becoming one itself.
