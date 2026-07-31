# Bounded YIN and polyphonic partial tracking

Load `sound-audio-lift` to call the public monophonic tracker. `pcm` is a map
with `:samples` (a mono sample vector) and `:sample-rate` (positive hertz):

```lisp
(sound/lift/pitch-track pcm
  :method 'pyin
  :range '(55.0 1760.0)
  :frames {:size 2048 :hop 256}
  :control {:work 500000 :results 8 :seed 0})
```

The callable returns the complete retained `PitchTrackPlan`: YIN or
probabilistic-YIN method, frequency range, threshold distribution, explicit
integer or parabolic interpolation, voiced-probability floor, final-frame
policy, work/result limits, and seed. Every frame returns accepted candidates,
rejected hypotheses, lower and upper frequency bounds, and its source PCM
location; the contour never substitutes a tuned note for its measured frequency.

`polyphonic_pitch_track` extends the existing spectral-peak and harmonic-comb
frames. It uses the shared discrete-graph minimum-cost assignment and DTW
implementations, retaining their work receipts while making births, deaths,
missing-frame tolerance, crossing, jump, live-track, memory, and aggregate-work
policy explicit. Generated silence, seeded noise, vibrato, missing-fundamental,
crossing-partial, and offset-tuning fixtures provide the checked evidence.
