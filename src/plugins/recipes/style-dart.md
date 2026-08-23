# Dart style

Follow Effective Dart and the repository formatter.

- Run `dart format`; use `UpperCamelCase` for types, `lowerCamelCase` for
  members, and `lowercase_with_underscores` for files and packages.
- Order `dart:` imports before package and relative imports, with exports in a
  separate sorted section.
- Preserve null safety. Prefer promotion and explicit nullable types over
  unnecessary `late` state or forced assertions.
- Use collection literals, `.isEmpty`, `for-in`, interpolation, tear-offs, and
  `final` for values that do not change.
- Keep public APIs small, type parameters meaningful, and fields direct rather
  than wrapping them in needless getters and setters.
- Write concise `///` documentation for public behavior and run analyzer and
  tests for the changed package.
