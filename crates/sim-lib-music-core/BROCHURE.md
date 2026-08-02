# sim-lib-music-core

In one line: The heart of the music world in SIM: notes, chords, melodies, and scores, plus the timeline they live on.

## What it gives you

This supplies the concrete music domain the rest of the stack works with. It defines the music object model -- notes, chords, melodies, and scores -- along with the piano roll and time grid they sit on, identity-bearing staffs, exact snapshots and change streams, events and lanes, players and playables, performances and takes, an arranger, freeze surfaces, and trace data. Its conversion lattice moves between catalog score forms with stable voice, note, and event identities; unrepresentable labels, rests, lanes, grids, or voice choices are returned as typed losses instead of disappearing.

## Why you will be glad

- Represent everything from a single note to a full score in one model.
- Lay music out on a shared timeline that every tool understands.
- Convert score forms without float time or silent information loss.
- Expose your music pieces to the runtime as named components.

## Where it fits

This is the trunk of the SIM music tree. Analysis, transformation, lifting, lowering, notation, and the generative players all grow from these types, so the whole constellation shares one idea of what a note, a chord, and a score are.
