# Music algorithm foundry

This recipe executes one `music/algorithm-plan` whose stages are selected from
loaded `music/algorithm-stage/<name>` exports by their argument Shapes. The
default data chooses SMF realization, key/chord/pitch/beat analysis,
scale/chord/dissonance listings, layered-DP harmony, first-species
counterpoint, and SMF/WAV rendering. Changing only `:strategy` selects the
independently registered exhaustive harmonizer.

Input and output are runtime Tables. A host may supply any Table/Dir backend;
the recipe never accepts a path, device, or provider endpoint. SMF is decoded
and encoded by `sim-lib-midi-smf`, audio is rendered offline by
`sim-lib-sound-render`, and optional audio input is owned by the loadable audio
lifter named in `setup.siml`. Realtime preview is a separate optional stage: it
accepts a caller-provided `realize` target and contains no device API. Omitting
that library reports the missing export and does not fall back to a hidden
preview implementation.

Run `cargo run -p music-algorithm-foundry`. The executable checks every
intermediate summary plus fixed final MIDI and WAV frame digests. Its tests also
prove the data-only alternate stage choice and the missing-preview diagnostic.
