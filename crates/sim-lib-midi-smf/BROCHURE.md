# sim-lib-midi-smf

In one line: Reads and writes ordinary .mid files, the standard way music moves between programs.

## What it gives you

This handles the on-disk Standard MIDI File format in both directions. It parses .mid and .smf bytes into an in-memory song model and serialises that model straight back to bytes, reusing the shared MIDI event types. It preserves all three file formats, metrical and SMPTE time divisions, unknown valid meta and system events, variable-length timing, and running-status compression. Defensive read limits bound files, tracks, chunks, events, and payload allocation before untrusted sizes are copied.

## Why you will be glad

- Open MIDI files exported by other music software.
- Save your work as a file any sequencer can load.
- Retain timecode files and independent format-2 patterns without pretending
  that they share a metrical timeline.
- Trust malformed or oversized input to fail closed with the exact byte offset.
- Trust canonical files to round-trip faithfully, including extension events.

## Where it fits

This is the file gateway of the SIM MIDI stack. It lets the constellation exchange songs with the wider world of sequencers and notation programs, feeding parsed files up to the lifting and analysis crates and taking arranged material back down to disk.
