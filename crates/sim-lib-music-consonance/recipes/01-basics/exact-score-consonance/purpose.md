# Inspect Exact Score Consonance

The report keeps every half-open score window, including the doubled C as a
separate event, and exposes pitch, acoustic, ratio, commonality, and leading
metrics under separate keys. Exact rational spans, note identities, velocity,
articulation, and provenance remain visible. The `aggregate` field is
deliberately nil: callers must name any later optimization objective.
