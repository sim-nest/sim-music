# Bounded MusicXML Partwise Profile

`sim-lib-music-notation` accepts one MusicXML exchange profile:
MusicXML 4.0 `score-partwise`. It is a notation profile over the canonical
`sim_lib_music_core::Score`, not a general XML codec and not a second score
model.

## Accepted domain

- Root: `score-partwise version="4.0"`, with one `part-list` and one or more
  referenced `part` elements.
- Global facts: positive integer tempo, fixed key signature in `fifths` plus
  `major` or `minor`, fixed time signature, and positive divisions no greater
  than 16,384.
- Structure: contiguous measures numbered from 1, complete under the active
  meter, with no event crossing a barline. Each part is one monophonic voice;
  multiple parts become canonical `Counterpoint`.
- Events: pitched notes and rests with positive exact rational durations.
  Optional type plus one dot must agree with the duration. Supported
  articulations are staccato, tenuto, accent, and strong accent.
- Identity: part and event `id` values use the conservative ASCII XML-id
  spelling `[A-Za-z_][A-Za-z0-9_.-]*`.

All other elements, attributes, namespaces, changing global facts, polyphonic
backup/forward notation, tuplets, ties/slurs, grace notes, chords, lyrics,
layout, mixed content, processing instructions, and extension markup fail
closed. Rust export supports `Note`, `Rest`, `Melody`, and named `Counterpoint`
score bodies. Other canonical music bodies return an explicit
unsupported-object error.

## Limits and parser policy

Every import applies independent ceilings for source bytes, XML nodes, element
depth, aggregate text, parts, and events. Runtime defaults are:

| Dimension | Default |
| --- | ---: |
| bytes | 4,000,000 |
| nodes | 200,000 |
| depth | 64 |
| text bytes | 1,000,000 |
| parts | 256 |
| events | 1,000,000 |

The implementation delegates XML tokenization and tree construction to
`roxmltree` 0.21.1. The dependency is used only for this bounded tree profile,
not exposed as a general XML facility. Review findings:

- license: dual MIT or Apache-2.0, compatible with this MPL-2.0 crate;
- implementation: `#![forbid(unsafe_code)]`;
- DTD policy: `ParsingOptions::allow_dtd = false`;
- entity resolver: absent, so external entity resolution is never installed;
- allocation: source bytes are checked first and the parser's `nodes_limit` is
  set before parsing; profile depth/text/part/event limits are then checked
  before score construction.

Built-in XML character references remain valid XML text. DTD declarations,
custom entities, external entities, and entity expansion are rejected.

## Identity and loss contract

Import returns canonical `Score` plus `NotationIdentity` records keyed by paths
such as `part/0/event/3`. Passing those records to report-based export
reproduces source ids; absent ids are allocated deterministically.

`NotationLoss` records every accepted notation fact that canonical `Score` or
this bounded output cannot carry. Current kinds cover clef layout, single-part
display names, enharmonic source spelling, defaulted tempo/meter, velocity, and
MIDI channel. Unsupported constructs are errors, never silent losses.

The runtime call

```lisp
(music/notation/import
  :format 'musicxml-partwise
  :source xml-bytes
  :limits {:bytes 4000000 :nodes 200000 :depth 64})
```

returns a map containing the existing `music/Score` citizen read-construct,
stable identity records, and loss records.
