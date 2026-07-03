# sim-lib-stream-file

In one line: Reads and writes streams to and from files on disk, with each access recorded and permission-checked.

## What it gives you

This provides source and sink adapters backed by the filesystem, so a stream can be fed from a file or written out to one. Every read and write stays behind an explicit file capability and is recorded as a filesystem effect, keeping disk access accountable rather than silent. It reuses the in-tree MIDI file codec for SMF support, and covers WAV audio through the stream audio layer.

## Why you will be glad

- Play a stream out of a file or capture one into a file.
- Keep every disk access permission-gated and recorded.
- Handle both MIDI files and WAV audio through one adapter.

## Where it fits

This is the disk end of the SIM streaming layer. It lets stream pipelines exchange data with stored files under the kernel's capability rules, sitting beside the MIDI and bridge stream adapters as the file-facing member of the family.
