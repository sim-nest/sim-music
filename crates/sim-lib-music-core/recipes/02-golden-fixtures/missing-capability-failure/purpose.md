# Missing Capability Failure (descriptor)

Documents a failure path (missing capability failure): the runtime failing closed when a capability is missing or
an operation is unsupported. Exercising it needs the music runtime, which is outside the
cookbook sandbox eval stack, so the fail-closed behavior is documented rather than run.
