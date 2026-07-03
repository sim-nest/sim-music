# sim-lib-sound-bridge

In one line: Connects MIDI notes to actual sound, deciding which instrument, tuning, and voice each note plays through.

## What it gives you

This is the junction between the MIDI world and the sound world. It consumes MIDI events and produces scheduled tones, resolving each note's program through a bank of timbres, its pitch through a tuning, and its polyphony through a pool of voices. Per-channel settings and state let it track and shape behaviour independently on each channel, and it installs as a runtime library so the whole bridge is available on demand.

## Why you will be glad

- Hear a MIDI stream through chosen instruments and tunings.
- Manage polyphony with a voice pool instead of note collisions.
- Shape each MIDI channel's behaviour on its own.

## Where it fits

This is the central connector of the SIM sound stack. It ties the MIDI model to the timbre, tuning, and rendering crates, turning abstract note events into scheduled tones ready to be voiced and rendered into audio.
