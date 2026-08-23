# C# style

Follow the repository formatter and nullable settings first.

- Use `PascalCase` for public types and members, `camelCase` for locals and
  parameters, and the established private-field convention.
- Declare access deliberately, keep one primary type per file, and order
  members consistently.
- Prefer immutable values, `const` or `readonly`, restrictive collection
  interfaces, and named option types over boolean argument puzzles.
- Use nullable annotations, pattern matching, interpolation, collection
  initializers, and async all the way through a call chain.
- Use LINQ when it clarifies intent, but avoid hidden allocation or repeated
  enumeration in hot paths.
- Keep methods focused, make ownership of mutable collections explicit, and
  test public behavior with the repository's standard test runner.
