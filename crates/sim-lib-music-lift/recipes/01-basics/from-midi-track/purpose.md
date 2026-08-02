# From MIDI Track (descriptor)

Documents the policy-controlled MIDI realization path: ordered tempo meta
events become exact tick/quarter-beat/wall-time charts; overlapping same-pitch
notes select FIFO, LIFO, or reject behavior; sustain, sostenuto, All Notes Off,
All Sound Off, and Reset All Controllers determine exact sounding releases; and
format-2 tracks remain independent patterns. The resulting note slices retain
one source identity per note-on, including equal-pitch unisons.

Music sequencing, synthesis, and notation run through the audio and render
pipeline outside the cookbook sandbox eval stack, so this descriptor is
documented rather than executed by the cookbook's core-only sandbox.
