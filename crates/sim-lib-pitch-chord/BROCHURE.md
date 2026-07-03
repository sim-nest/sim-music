# sim-lib-pitch-chord

In one line: Builds chords from notes, scale degrees, or jazz symbols, voices them, and can harmonize a melody for you.

## What it gives you

This is the chord workshop. It builds chords from raw pitches, from scale degrees, or from jazz-style symbols, then reshapes them with voicing and velocity policies to sit and sound the way you want. Generative players harmonize incoming pitches against a chosen scale, and on top sit a wire-serializable chord-progression sequencer and a roman-numeral-aware harmony suggester that proposes what might come next.

## Why you will be glad

- Spell a chord from a jazz symbol like Cmaj7 in one step.
- Voice and shape chords so they land where you want them.
- Get harmony suggestions to carry a progression forward.

## Where it fits

This is the harmony layer of the SIM pitch family. It builds on the core pitch types and the scale crate to turn single notes into full chords and progressions, feeding the players, analysis, and naming tools that reason about harmony.
