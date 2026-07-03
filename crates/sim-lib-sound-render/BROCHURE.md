# sim-lib-sound-render

In one line: Turns synthesized tones into real audio samples and writes them out as a WAV file.

## What it gives you

This renders tones into playable audio. It takes synthesized tones and produces interleaved PCM samples, rendering single tones and mixing scheduled tones with per-tone timing and panning, then encodes the mix as a standard 16-bit WAV. Options set the sample rate and channel count. It is the step that finally turns a described sound into something a speaker can play or a file can store.

## Why you will be glad

- Bounce synthesized tones straight to a WAV file.
- Mix scheduled tones with their own timing and panning.
- Set sample rate and channels to match your target.

## Where it fits

This is the output stage of the SIM sound family. It sits at the end of the chain after the bridge and instruments have decided what should sound, converting scheduled tones into finished audio the DAW session, previews, and listeners can use.
