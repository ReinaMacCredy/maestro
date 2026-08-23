# TypeScript style

Follow the repository formatter and strict compiler settings. In this project,
use bun for runtime and tests.

- Use ES modules, named exports, `const` by default, strict equality, and
  explicit return types on exported APIs.
- Model multi-state data with discriminated unions and exhaustively handle the
  discriminant. Use const objects instead of enums when simple values suffice.
- Parse untrusted input once at the boundary. Inside the boundary, trust the
  parsed type instead of repeating checks.
- Prefer `unknown` plus narrowing over `any`; prefer `satisfies` over unchecked
  assertions. Avoid non-null assertions and optional fields that create illegal
  combinations.
- Let inference describe local implementation details, but keep public
  contracts stable and narrow.
- Test observable behavior with `bun:test`, then run the repository's type and
  build checks for the changed surface.
