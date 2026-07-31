# sim-lib-music-combinators

In one line: A shelf of generative players -- arpeggiators, basslines, drum patterns, step sequencers -- that turn simple inputs into steady streams of notes.

## What it gives you

This layers reusable players on top of the core music types. Feed each one musical raw material -- chords, scales, drum kits, step lanes -- and it renders a deterministic stream of play events with matching trace data, so the same settings always give the same performance. The collection covers arpeggiation in a couple of flavours, walking basslines, drum patterns including a Euclidean generator, polyphonic step sequencing, and multi-stream note generation. It also joins editable harmony rules to the named pitch, ratio, sonance, and voice-leading measures, then applies exhaustive, factored, certified layered, or beam planning without hiding bounds, failures, cost, or optimality evidence.

## Why you will be glad

- Generate arps, basslines, and beats from a few settings.
- Get the same performance every run, which makes results repeatable.
- Assemble the source chords and scales with tidy builder helpers.
- Compare harmony choices with named musical models while retaining their evidence.
- Prove the optimum on small phrases and return honest bounded receipts on larger ones.

## Where it fits

This is the generative engine room of the SIM music family. It sits above the core model and produces the actual note streams that instruments and renderers play, giving composers and agents a set of dependable pattern makers to drive an arrangement.
