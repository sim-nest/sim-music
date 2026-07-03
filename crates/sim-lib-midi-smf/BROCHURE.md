# sim-lib-midi-smf

In one line: Reads and writes ordinary .mid files, the standard way music moves between programs.

## What it gives you

This handles the on-disk Standard MIDI File format in both directions. It parses .mid and .smf bytes into an in-memory song model and serialises that model straight back to bytes, reusing the shared MIDI event types. It covers all three file formats, the variable-length timing encoding, running-status compression, and cleaning up or merging tracks. Timing is read in the common ticks-per-quarter form; the rarer SMPTE timing is refused rather than mishandled.

## Why you will be glad

- Open MIDI files exported by other music software.
- Save your work as a file any sequencer can load.
- Trust the round-trip, since bytes in and bytes out stay faithful.

## Where it fits

This is the file gateway of the SIM MIDI stack. It lets the constellation exchange songs with the wider world of sequencers and notation programs, feeding parsed files up to the lifting and analysis crates and taking arranged material back down to disk.
