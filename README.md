# sim-music

Gives you the whole world of music as first-class Rust objects -- pitches,
chords, scales, notes, MIDI, tunings, timbres, and synthesized sound -- that you
can build, transform, analyze, and render.

## Example

```console
$ cargo add sim-lib-pitch-core
```

```rust
use sim_lib_pitch_core::PitchClass;

// Transpose C up a perfect fifth (7 semitones); measure an interval class.
assert_eq!(PitchClass::C.transpose(7), PitchClass::G);
assert_eq!(PitchClass::E.interval_class(PitchClass::C), 4);
```

`PitchClass` is a mod-12 pitch class (`C = 0` .. `B = 11`), the foundational
primitive shared across the pitch crates. (Doctest:
`crates/sim-lib-pitch-core/src/model.rs:27`.)

The full SIM walkthrough, including how to run the `sim` CLI (`cargo install
sim-run`), lives in `sim-say`.

## How it works

`sim-music` is the music and audio domain of the SIM constellation. SIM is an
expandable Rust runtime built around a small protocol kernel plus a large set of
loadable libraries: the kernel defines contracts, libraries provide behavior.
This repository holds the libraries that give SIM a concrete music domain --
pitch theory, the music object model, sound synthesis, MIDI, and the codec and
streaming surfaces that connect them to the rest of the runtime.

The crates layer from primitives upward. Pitch theory crates model pitches,
sets, scales, and chords; the music crates build notes, melodies, scores, and
generative players over them; the sound crates render tones, timbres, tunings,
and PCM audio; and the MIDI crates carry the standard wire and file formats.
Each domain exposes its types to the runtime as `Shape` values, citizen
descriptors, text codecs, and frame-safe descriptors, and registers them as
loadable SIM libs.

## Crates

### Pitch theory

- `sim-lib-pitch-core` -- foundational pitch primitives: the mod-12
  `PitchClass`, octave-aware `Pitch`, letter-plus-accidental `SpelledPitch`, and
  the semitone `Interval` shared across the pitch crates.
- `sim-lib-pitch-set` -- unordered pitch collections as compact bitmasks:
  `PitchClassMask`, prime-form normalization, the `IntervalVector` census,
  `PitchRangeMask`, `BitChord`, and `ThirdStackSignature`.
- `sim-lib-pitch-scale` -- diatonic and symmetric `Mode`s, the `Scale` type, and
  scale-locking `PlayerScale`/`ScaleLockPlayer` performance surfaces.
- `sim-lib-pitch-chord` -- chords, voicings, and harmonic sequencing: `Chord`,
  `ChordSymbol`, voicing/velocity policies, generative harmonizing players, and
  a roman-numeral harmony suggester.
- `sim-lib-pitch-dissonance` -- dissonance and harmonic-complexity scoring of
  pitch-class sets over a registry of pluggable `PitchDissonanceModel`s, exposed
  as a runtime lib.
- `sim-lib-pitch-shapes` -- the pitch codec and runtime surface: string
  round-trips, citizen descriptors, and `Shape` values for pitches, intervals,
  masks, scales, keys, and chords.
- `sim-lib-pitch-wasm-frame` -- frame-safe pitch descriptors for web and wasm ABI
  adapters.

### Pitch naming

- `sim-lib-pitch-namer` -- aggregator over the naming schools: the
  `ClusterNamer` trait, `NamingSchool` taxonomy, and a `NamerRegistry` that
  labels a set in every school at once and translates labels between schools.
- `sim-lib-pitch-namer-forte` -- the Forte set-class naming school, mapping a
  pitch-class set to its Forte name via a prime-form lookup table.
- `sim-lib-pitch-namer-jazz` -- the jazz school: parses, realizes, and matches
  jazz chord symbols such as `Cmaj7` or `Am7/G`.
- `sim-lib-pitch-namer-riemann` -- the neo-Riemannian / functional triad school,
  labeling major and minor triads by functional quality.
- `sim-lib-pitch-namer-roman` -- the functional roman-numeral school, labeling a
  chord by scale degree and quality within a key.

### Music model, transforms, and analysis

- `sim-lib-music-core` -- the core music object model: notes, chords, melodies,
  scores, descriptors, events and lanes, the piano roll and time grid, players
  and playables, performances, the arranger, and the component registry.
- `sim-lib-music-combinators` -- composable generative players and combinators
  over the core material types: arpeggiators, basslines, drum patterns,
  polyphonic step sequencing, and multi-stream note generation.
- `sim-lib-music-transform` -- transformations on musical material: transpose,
  invert, retrograde, augment, diminish, pitch/time remaps, pattern mutators,
  and a capability-gated custom event filter pipeline.
- `sim-lib-music-analysis` -- structural views over music objects: the
  `DiffRoll` per-event analysis, `ChordWindow` segmentation, and optional
  Walsh-Hadamard spectral analysis of melodies and contours.
- `sim-lib-music-lift` -- lifts low-level representations (parsed MIDI/SMF) into
  richer music: piano rolls, diff rolls, chord progressions, and counterpoint,
  each with a diagnostic `LiftReport`.
- `sim-lib-music-lower` -- the inverse of lifting: renders a structured music
  object or `Score` down to a playable Standard MIDI File and serialized bytes.
- `sim-lib-music-notation` -- the notation codec surface, converting between a
  `Score` and a LilyPond-subset text rendering through `NotationCodec`.
- `sim-lib-music-shapes` -- citizen descriptors, the `#(...)` text codec, and a
  loadable lib that registers the music types as documented `Shape` values.
- `sim-lib-music-wasm-frame` -- frame-safe music descriptors and stable wasm engine
  entrypoint names for browser and ABI adapters.

### Sound and synthesis

- `sim-lib-sound-core` -- foundational acoustic primitives: `Frequency`,
  `Amplitude`, `Phase`, and the `Partial`/`Envelope`/`Tone` spectral-tone model.
- `sim-lib-sound-spectrum` -- the `Spectrum` frequency-domain representation and
  its descriptors: peaks, centroid, flatness, rolloff, and flux.
- `sim-lib-sound-timbre` -- `Timbre` synthesis recipes (sine, sawtooth, square,
  triangle, organ pipe, Karplus-Strong, FM, inharmonic bell), the spectral
  `Filter` family, and a runtime lib of built-in timbres.
- `sim-lib-sound-tuning` -- the `Tuning` trait and concrete temperaments (equal,
  just, Pythagorean, meantone, Werckmeister III, Young, Scala cents), with a
  serializable `TuningDescriptor` and runtime lib.
- `sim-lib-sound-dissonance` -- the `DissonanceModel` trait and sensory
  estimators (Plomp-Levelt, Sethares, Helmholtz, harmonic entropy) with a
  lookup registry and runtime lib.
- `sim-lib-sound-gm` -- the General MIDI sound set: the GM drum map and a bank
  mapping the 128 GM melodic programs onto concrete timbres.
- `sim-lib-sound-bridge` -- MIDI-to-sound bridging: consumes MIDI events and
  produces `ScheduledTone`s, resolving programs, tunings, and polyphony.
- `sim-lib-sound-render` -- renders synthesized tones into interleaved PCM,
  mixing scheduled tones and encoding 16-bit WAV.
- `sim-lib-sound-audio-lift` -- audio-to-notes lifting: analyzes raw PCM and
  lifts it into pitched note candidates via FFT-peak and harmonic-comb lifters.
- `sim-lib-sound-shapes` -- the sound-layer text codec: `#(...)` round-trips for
  the sound types, citizen descriptors, and a runtime lib.
- `sim-lib-sound-wasm-frame` -- frame-safe sound descriptors and stable wasm engine
  entrypoint names for browser and ABI adapters.
- `sim-lib-music-synth` -- playable pure-Rust synthesizer primitives
  (oscillators, filters, envelopes, LFOs, voice allocation) and several modeled
  instruments (DX7-style FM, PS-3300, System 55, System 700) over the shared
  audio graph.

### MIDI

- `sim-lib-midi-core` -- the protocol-agnostic MIDI data model: tick timing,
  bounded byte domains, the event model, and the `MidiSource`/`MidiSink` traits
  with in-memory implementations.
- `sim-lib-midi-smf` -- Standard MIDI File reading and writing over the core
  event types, covering the three SMF formats, VLQ, running status, and track
  canonicalization.
- `sim-lib-midi-sysex` -- typed views over system-exclusive messages: Universal
  SysEx, the MIDI Tuning Standard, and Yamaha/DX7 voice and bank formats.
- `sim-lib-midi-live` -- real-time MIDI ring buffers bridging live I/O into the
  source/sink traits, dropping and counting overflow rather than blocking.
- `sim-lib-midi-rtmidi` -- the RtMidi-facing adapter surface with deterministic
  fake ports, source/sink wrappers, and host backend inventory behind a stable
  API.
- `sim-lib-midi-ble` -- BLE-MIDI discovery and bridge metadata for MD-BT01 class
  devices from fixture data.
- `sim-lib-midi-shapes` -- the MIDI string codec and citizen descriptors that
  round-trip MIDI events, tracks, and SMF files through the `#(...)` form, with
  a loadable lib.
- `sim-lib-midi-wasm-frame` -- frame-safe MIDI descriptors for web and wasm ABI
  adapters.

### Stream adapters

- `sim-lib-stream-midi` -- adapts the MIDI core source/sink traits into
  stream-core MIDI packets.
- `sim-lib-stream-file` -- file-backed source and sink adapters behind explicit
  stream file capabilities, reusing the in-tree SMF and PCM16 WAV codecs.
- `sim-lib-stream-bridge` -- adapts finite stream packet spines between MIDI and
  PCM using the sound and audio-lift libraries.

## Lift and lower

The music domain is organized around a single reversible pipeline. A *lift*
raises a concrete, low-level representation into a richer structured one: a
parsed Standard MIDI File becomes a `PianoRoll`, a `DiffRoll` analysis view, a
chord `Progression`, or a `Counterpoint` of separated voices, with diagnostics
recording any lossy or ambiguous decision. A *lower* runs the same path in
reverse, rendering a structured music object or `Score` back down to a playable
Standard MIDI File. Transforms operate on the canonical `PianoRoll` in the
middle, so analysis, transformation, and rendering all share one model.
`sim-lib-sound-audio-lift` extends the same idea below MIDI, lifting raw PCM
audio into pitched note candidates that feed the music layer.

## Validation

Install the native MIDI build metadata before running the full all-features
gate. On Debian or Ubuntu:

```sh
sudo apt-get install -y pkg-config libasound2-dev
```

Run the same gates as CI:

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
cargo run -p xtask -- simdoc --check
```

## Documentation Lanes

`cargo run -p xtask -- simdoc` builds the public documentation lanes:

- API docs: `target/doc/`
- Agent cards: `docs/agents/cards.jsonl` and `docs/agents/card-index.json`
- Human docs: `docs/humans/`
- Diagrams: `docs/diagrams/src/` and `docs/diagrams/generated/`

The same command writes split contract files under `docs/generated/`.
