# Certified Tonnetz Path (descriptor)

The public call shape is:

```lisp
(music/analyze/tonnetz
  :from (chord "C:maj")
  :to (chord "A:min")
  :moves '(P L R)
  :limit 8)
```

`analyze_tonnetz` reduces both chords to rooted pitch-class identities and
applies the enabled P/L/R generators over the finite 24-node major/minor
Tonnetz. It delegates shortest-path solving and certificate verification to
`sim-lib-discrete-graph`. The fixture above has the unique one-move result `R`.

Riemannian labels are projected through `sim-lib-pitch-namer-riemann` only after
the identity path exists. Enharmonic spelling, octave, voicing, slash-bass text,
and rendered labels never determine a move or graph node.
