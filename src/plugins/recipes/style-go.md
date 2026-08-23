# Go style

Follow `gofmt` and the repository's package conventions.

- Use short lowercase package names and `MixedCaps` identifiers. Let initial
  capitalization express export visibility.
- Keep interfaces small and define them at the consumer when practical.
- Return values and errors explicitly; add actionable context and never discard
  an error without a reason. Reserve panic for unrecoverable programmer faults.
- Prefer clear control flow, early returns, `for range`, and `defer` for local
  cleanup.
- Use slices, maps, and channels with explicit ownership. Introduce goroutines
  only with a clear lifetime, cancellation path, and synchronization contract.
- Avoid getters named `GetX`, oversized abstractions, and shared mutable state.
- Run formatting, vet or configured lint, race-sensitive checks, and tests.
