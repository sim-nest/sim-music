# Serial workbench

This executable is the public end-to-end serial music recipe. It assembles one
immutable row plan, realizes it strictly and through a replaceable modal
realizer, completes the modal result with one additive patch, audits the
structural and sounding ledgers, reverses the patch, exports MIDI plus
LilyPond notation, and lowers an audition score.

Run `cargo run -p serial-workbench`. The program validates its fixture manifest
before doing any work and then checks every stage result in process.
