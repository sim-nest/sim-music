# Pitch-Set Neighborhood Walk (descriptor)

Documents pitch-set geometry as ordinary discrete graph data. The fixture keeps
cluster, trail, and round evidence as `sim-lib-discrete-graph` shortest-path
certificates; it does not define a private pitch-set BFS.

The source API builds interval and cyclic gap forms, names the zero-gap
multiplicity policy, materializes jumping or non-jumping neighborhoods under
`SearchControl`, and verifies graph paths through reusable shortest-path
certificates.
