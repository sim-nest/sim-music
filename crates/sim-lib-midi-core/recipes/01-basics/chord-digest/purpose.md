# Digest an offline MIDI chord render

A Category C recipe: the effectful artifact (a MIDI note stream) is reduced to a
DETERMINISTIC digest. `midi/chord-digest` encodes a C-major triad to its
canonical MIDI wire bytes offline -- no device, no clock, no entropy -- and
returns the integer FNV-1a `frame` digest of those bytes. Because the bytes are
fixed by the MIDI spec and the digest is integer-only, two runs reproduce
byte-for-byte, which the cookbook twice-run guard asserts.
