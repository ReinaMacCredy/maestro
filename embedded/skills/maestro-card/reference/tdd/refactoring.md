# Refactor Candidates

After TDD cycle, look for:

- **Duplication** → Extract function/class
- **Long methods** → Break into private helpers (keep tests on public interface)
- **Shallow modules** → Combine or deepen
- **Feature envy** → Move logic to where data lives
- **Primitive obsession** → Introduce value objects
- **Existing code** the new code reveals as problematic

On a maestro card this refactor step IS the simplify pass
([../simplify.md](../simplify.md)) -- run it once here, not again before
`task complete`. Simplify widens these candidates with the reuse, altitude,
and dead-code lenses.
