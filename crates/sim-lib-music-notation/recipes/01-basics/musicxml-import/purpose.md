# Bounded MusicXML Import

Use the one notation callable with the `musicxml-partwise` profile:

```lisp
(music/notation/import
  :format 'musicxml-partwise
  :source xml-bytes
  :limits {:bytes 4000000 :nodes 200000 :depth 64})
```

The callable is Shape-described and returns the existing `music/Score`
read-construct together with stable part/event ids and every accepted loss.
DTDs, entity declarations, unknown extensions, and resource overruns fail
closed. The Rust conformance test supplies the XML bytes because the cookbook
sandbox does not expose host file reads.
