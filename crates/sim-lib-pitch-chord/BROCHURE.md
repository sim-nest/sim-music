# sim-lib-pitch-chord

In one line: Builds chords from notes, scale degrees, or jazz symbols, voices them, and can harmonize a melody for you.

## What it gives you

This is the chord workshop. It builds chords from raw pitches, from scale degrees, or from jazz-style symbols, then reshapes them with voicing and velocity policies to sit and sound the way you want. Chord and voicing palettes remove exact duplicates while keeping stable identities. Complete harmony programs stay as editable data: cadence chains, hard rules, weighted preferences, voice changes, and render settings travel together with evidence for every decision. Generative players harmonize incoming pitches against a chosen scale, and a chord-progression sequencer plus a roman-numeral-aware suggester carries a progression forward.

## Why you will be glad

- Spell a chord from a jazz symbol like Cmaj7 in one step.
- Voice and shape chords so they land where you want them.
- Build deterministic, duplicate-free voicing palettes for downstream exact voice-leading plans.
- Edit a harmony vocabulary and its rules as data without rebuilding the library.
- Inspect why every hard rule passed or failed separately from musical preference scores.
- Get harmony suggestions to carry a progression forward.

## Where it fits

This is the harmony layer of the SIM pitch family. It builds on the core pitch types and the scale crate to turn single notes into full chords and progressions, feeding the players, analysis, and naming tools that reason about harmony.
