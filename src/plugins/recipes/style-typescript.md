# TypeScript style

Follow the repository formatter and strict compiler settings. In this project,
use bun for runtime and tests. Apply **Type System Discipline** (PRINCIPLES.md)
first. Folds rules from pstack's typescript-best-practices (Lauren Tan, MIT;
notice in THIRD_PARTY.md).

- Use ES modules, named exports, `const` by default, strict equality, and
  explicit return types on exported APIs.
- Model multi-state data with discriminated unions and exhaustively handle the
  discriminant. Use const objects instead of enums when simple values suffice.
- Keep the simplest total type. Leave `T[]` while every operation on it is
  total. Strengthen it to a non-empty `[T, ...T[]]` only where the loose type
  forces `!`, a cast, or a "should never happen" throw.
- Parse untrusted input once at the boundary. Inside the boundary, trust the
  parsed type instead of repeating checks.
- Brand a primitive that must not be mixed up with another, in the shape
  `string & { readonly __brand: "AgentId" }`, and mint it only in the boundary
  parser that validates it. Keep that brand shape; do not invent another.
- Prefer `unknown` plus narrowing over `any`; prefer `satisfies` over unchecked
  assertions. Avoid non-null assertions and optional fields that create illegal
  combinations.
- Narrow in this order: a discriminant switch, then `in`, then `typeof` or
  `instanceof`, then a user-defined type guard, and `as` only after validation.
- A type guard verifies the whole claim its name makes and is named `isX` or
  `hasX`. A lying guard is worse than `as`, because the bug hides behind a
  name that says the value is safe.
- Let inference describe local implementation details, but keep public
  contracts stable and narrow.
- Pass one object instead of positional arguments, so each value is named at
  the call. Keep positional arguments on hot paths (per-frame render,
  tokenizers, parsers).
- Ship diagnostics through the program's structured logger, with enough
  context to debug from an id, never a stray `console.log`. A CLI's own stdout
  is its product, not telemetry.
- Test observable behavior with `bun:test`, then run the repository's type and
  build checks for the changed code.
