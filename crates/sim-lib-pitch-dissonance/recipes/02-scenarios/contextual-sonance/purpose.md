# Contextual Sonance (descriptor)

Documents a contextual sonance comparison over voiced pitch events. The cookbook
sandbox does not load the full pitch, ratio, and registry stack, so this recipe
is a descriptor rather than an executable eval.

The source API retains input identity and duplicate notes, applies typed
configuration for context window, voice identity, weighting, normalization, and
merge policy, and returns one component per named model instead of flattening
the report into one scalar.
