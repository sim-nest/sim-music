# Unsupported Export Failure (descriptor)

Documents a failure path (unsupported export failure): the runtime failing closed when a capability is missing or
an operation is unsupported. Exercising it needs the midi runtime, which is outside the
cookbook sandbox eval stack, so the fail-closed behavior is documented rather than run.
