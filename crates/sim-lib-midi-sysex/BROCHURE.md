# sim-lib-midi-sysex

In one line: Makes sense of the maker-specific MIDI messages that carry synth patches and tuning tables, turning raw bytes into readable settings.

## What it gives you

System-exclusive messages are the private channel where instruments send patches, tunings, and device settings as opaque bytes. This crate reads those payloads as structured, named messages and writes them back. It covers the universal messages every device shares, the MIDI Tuning Standard, and Yamaha's own messages including full DX7 voice and bank patches, with packed and unpacked voice conversion and correct checksums. Every data byte is validated, so bad input is flagged instead of trusted.

## Why you will be glad

- Read a DX7 patch dump as named settings, not a wall of bytes.
- Send and receive tuning tables through the standard message form.
- Rely on checked bytes and checksums to catch corrupt data.

## Where it fits

This is the deep-decoder layer of the SIM MIDI stack. Where the core model treats system-exclusive data as an opaque blob, this crate gives it meaning, feeding synth patches and tunings into the sound and instrument crates that can act on them.
