# sim-lib-music-analysis

In one line: Looks at a piece of music and reveals its structure -- what chords are sounding, where the pulse is, how passages relate, and which patterns return.

## What it gives you

This studies music material and produces structural views you can read. It turns a piano roll into per-moment frames showing which pitches are sounding, starting, ending, or held over, then segments that timeline into chord-bearing stretches with pitch ranges and pitch-class masks. Its harmonic feature adapter decodes built-in key/chord profiles or caller-declared templates through the shared finite HMM, retaining transition rows, likelihood, confidence, posterior alternatives, and bounded-work evidence. Exact metrical quantization globally aligns identified notes to a declared tempo, meter, swing, and tuplet lattice without hiding any movement. Named melody and rhythm extractors compare passages through reusable dynamic-time-warp and correlation engines while stating transposition and time-scale invariances. Bounded repeated-pattern discovery hashes candidates, exact-verifies them, and returns occurrence identities, spans, transforms, costs, support, overlap policy, and search receipts. Its Tonnetz analyst finds bounded P/L/R paths between canonical major and minor chord identities with reusable shortest-path certificates, while keeping Riemannian names as display-only projections. With its spectral option it adds a Walsh-Hadamard analysis of melodies, contours, and pitch-class windows, giving another angle on repetition and shape.

## Why you will be glad

- See the chord behind each moment of a passage automatically.
- Track exactly when notes begin, end, or carry across.
- Get a spectral read on a melody's shape and repetition.
- Decode changing keys or chords without hiding alternative readings or posterior evidence.
- Quantize against explicit straight, swung, and tuplet grids with exact-time edits.
- Compare transposed or uniformly stretched melodies and rhythms with inspectable costs.
- Discover repeated passages under explicit support, overlap, memory, and work bounds.
- Find reproducible neo-Riemannian chord paths without treating names as identities.

## Where it fits

This is the listening-and-understanding layer of the SIM music family. It reads the core music model and hands its structural findings to naming, harmony, and transformation tools, so the rest of the stack can reason about what is actually happening in a piece rather than just its raw notes.
