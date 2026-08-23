# Rust style

Follow `rustfmt`, Clippy, and the repository's minimum supported Rust version.

- Use conventional casing and expose the minimum public surface. Prefer shallow
  modules and curated re-exports.
- Borrow instead of cloning when ownership is unnecessary. Use newtypes and
  enums to make invalid states unrepresentable.
- Return `Result` for recoverable failures, propagate with `?`, and attach
  actionable context. Reserve panic and unwrap for proven invariants or tests.
- Prefer standard conversion traits, small concrete types, exhaustive pattern
  matching, iterators, and `Option` over sentinels.
- Avoid premature traits, generics, boxes, and shared ownership.
- Keep unit tests near private behavior and integration tests at public seams;
  run format, Clippy, and the relevant cargo tests.
