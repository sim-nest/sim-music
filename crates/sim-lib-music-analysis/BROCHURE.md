# sim-lib-music-analysis

In one line: Looks at a piece of music and reveals its structure -- what chords are sounding and how the notes move moment to moment.

## What it gives you

This studies music material and produces structural views you can read. It turns a piano roll into per-moment frames showing which pitches are sounding, starting, ending, or held over, then segments that timeline into chord-bearing stretches with pitch ranges and pitch-class masks. Its harmonic feature adapter decodes built-in key/chord profiles or caller-declared templates through the shared finite HMM, retaining transition rows, likelihood, confidence, posterior alternatives, and bounded-work evidence. Its Tonnetz analyst finds bounded P/L/R paths between canonical major and minor chord identities with reusable shortest-path certificates, while keeping Riemannian names as display-only projections. With its spectral option it adds a Walsh-Hadamard analysis of melodies, contours, and pitch-class windows, giving a different angle on repetition and shape.

## Why you will be glad

- See the chord behind each moment of a passage automatically.
- Track exactly when notes begin, end, or carry across.
- Get a spectral read on a melody's shape and repetition.
- Decode changing keys or chords without hiding alternative readings or posterior evidence.
- Find reproducible neo-Riemannian chord paths without treating names as identities.

## Where it fits

This is the listening-and-understanding layer of the SIM music family. It reads the core music model and hands its structural findings to naming, harmony, and transformation tools, so the rest of the stack can reason about what is actually happening in a piece rather than just its raw notes.
