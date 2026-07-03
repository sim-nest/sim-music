# sim-lib-pitch-scale

In one line: Defines scales and modes and can snap incoming notes onto the scale you choose.

## What it gives you

This gives you scales and modes as working objects. It defines the diatonic and symmetric modes, anchors a mode to a tonic to make a concrete scale, and provides the diatonic operations built on them -- degree lookup, moving notes up and down within the scale, and mapping chord tones to scale tones. Performance-oriented players go further, quantizing, filtering, or remapping incoming pitches onto a chosen scale so a part stays in key.

## Why you will be glad

- Build any mode on any tonic as a ready-to-use scale.
- Move notes within a key by scale degree, not raw semitones.
- Lock a live part onto a chosen scale automatically.

## Where it fits

This is the scale layer of the SIM pitch family. It sits above the core pitch types and under the chord and harmony tools, giving the constellation a shared idea of keys and modes that players, namers, and generators all rely on.
