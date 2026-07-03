# sim-lib-music-lower

In one line: Takes a structured piece of music and renders it back down into a playable MIDI file.

## What it gives you

Lowering is the reverse of lifting. This crate takes a structured music object or a full score and turns it into a concrete, playable Standard MIDI File, then serialises that file to bytes ready to save or send. Options let you set the timing resolution, supply a tempo map, and decide how voices are spread across MIDI tracks, so the rendered file lands in the shape a sequencer or player expects.

## Why you will be glad

- Render a finished score into a file any sequencer can play.
- Control timing resolution and tempo as the piece comes down.
- Choose how voices split across tracks in the output file.

## Where it fits

This is the downward ramp of the SIM music stack, mirroring the lifting crate. Where lifting pulls meaning up out of MIDI, this pushes finished music back down to a playable file, closing the loop between structured composition and something you can actually hear.
