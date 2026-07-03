# sim-lib-music-combinators

In one line: A shelf of generative players -- arpeggiators, basslines, drum patterns, step sequencers -- that turn simple inputs into steady streams of notes.

## What it gives you

This layers reusable players on top of the core music types. Feed each one musical raw material -- chords, scales, drum kits, step lanes -- and it renders a deterministic stream of play events with matching trace data, so the same settings always give the same performance. The collection covers arpeggiation in a couple of flavours, walking basslines, drum patterns including a Euclidean generator, polyphonic step sequencing, and multi-stream note generation, plus friendly builders for assembling the music objects they read.

## Why you will be glad

- Generate arps, basslines, and beats from a few settings.
- Get the same performance every run, which makes results repeatable.
- Assemble the source chords and scales with tidy builder helpers.

## Where it fits

This is the generative engine room of the SIM music family. It sits above the core model and produces the actual note streams that instruments and renderers play, giving composers and agents a set of dependable pattern makers to drive an arrangement.
