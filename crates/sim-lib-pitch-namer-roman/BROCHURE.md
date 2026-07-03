# sim-lib-pitch-namer-roman

In one line: Reads a chord in the context of a key and names its scale degree -- I, V7, and so on -- the way theory class does.

## What it gives you

This implements the functional roman-numeral naming school. Given a key, it labels a chord by its scale degree from one through seven and by its quality, using upper-case numerals for major chords, lower-case for minor, a small circle for diminished, and seventh suffixes -- so a dominant seventh in a major key reads as V7. It needs a key to work against and returns a plain diagnostic when a chord will not fit.

## Why you will be glad

- Name a chord's role in a key with a familiar roman numeral.
- Capture quality and sevenths in the label, not just the degree.
- Get a clear message when a chord does not fit the key.

## Where it fits

This is the roman-numeral voice in the SIM pitch-naming family. It brings classroom harmonic analysis to the naming aggregator, alongside the Forte, jazz, and Riemannian schools, so a progression can be read in functional terms relative to its key.
