# Python style

Follow the repository formatter, type checker, and supported Python version.

- Use `snake_case` for functions and variables, `PascalCase` for classes, and
  uppercase names for module constants.
- Group imports as standard library, third-party, then local. Avoid mutable
  default arguments and mutable global state.
- Type public APIs, use built-in exception classes, add context at boundaries,
  and never use a bare `except`.
- Prefer clear loops over dense comprehensions, `is None` for `None`, context
  managers for resources, and f-strings for readable formatting.
- Document public behavior concisely; comments explain why. Keep executable
  entrypoints behind a `main()` function and guard.
- Run the configured formatter, linter, type checker, and tests.
