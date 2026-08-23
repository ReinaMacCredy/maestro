# C++ style

Follow the repository formatter and target standard first. For new code:

- Prefer C++20 portable features, direct includes, self-contained headers, and
  clear namespaces. Do not rely on transitive includes.
- Use `PascalCase` for types, `snake_case` for variables, and the project's
  established function convention.
- Express single ownership with values or `std::unique_ptr`; use shared
  ownership only when it is real. Prefer composition over inheritance.
- Make single-argument constructors explicit, define copy and move intent, and
  use factories for fallible construction.
- Prefer return values, `std::optional`, and small interfaces over output
  parameters and wide classes.
- Use `nullptr`, C++ casts, range loops, `constexpr`, and RAII. Avoid macros,
  hidden globals, and premature templates.
- Handle every failure under the project's exception policy and verify with
  the configured formatter, compiler warnings, and tests.
