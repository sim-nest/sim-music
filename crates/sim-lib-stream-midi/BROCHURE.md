# sim-lib-stream-midi

In one line: Wraps live MIDI sources and sinks so they plug into the SIM streaming pipeline as ordinary packets.

## What it gives you

This adapts the existing MIDI memory, source, and sink pieces into the stream system's packet form, so MIDI can flow through a streaming pipeline like any other material. It keeps to the shared MIDI contracts rather than adding new device backends, so it works with whatever MIDI plumbing is already in place. It is the connector that lets MIDI join the stream world without changing how MIDI itself behaves.

## Why you will be glad

- Run MIDI through a streaming pipeline as normal packets.
- Reuse the MIDI sources and sinks you already have.
- Keep MIDI behaviour unchanged while it joins the stream layer.

## Where it fits

This is the MIDI on-ramp for the SIM streaming layer. It sits between the MIDI core and the stream system, feeding note events into pipelines that the bridge and file adapters also serve, so MIDI travels the same rails as audio and files.
