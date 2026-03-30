# AGENTS.md
# TypeScript Style Guide

## Types
- Prefer `interface` for object shapes and `type` for unions or intersections
- Avoid `any`; use `unknown` and narrow with type guards
- Use `readonly` for immutable data
- Prefer `const` assertions for literal types
- Use discriminated unions over optional fields for variant types

## Naming
- Types and interfaces: PascalCase
- Variables and functions: camelCase
- Constants: UPPER_SNAKE_CASE
- Enums: PascalCase for both enum names and members
- Files: kebab-case

## Functions
- Prefer arrow functions for callbacks and short expressions
- Use named functions for top-level declarations
- Add explicit return types for public API functions
- Use function overloads sparingly; prefer union types

## Async
- Always `await` promises; avoid fire-and-forget flows
- Use `Promise.all()` for parallel independent operations
- Handle errors with `try/catch` at the boundary rather than every call site
- Prefer `async/await` over `.then()` chains

## Imports
- Group imports by built-in, external, internal, then relative
- Use named imports instead of `import *`
- Avoid circular dependencies

## Nullability
- Prefer `undefined` over `null`
- Use optional chaining (`?.`) and nullish coalescing (`??`)
- Avoid non-null assertions except in tests or tightly constrained cases

## Testing
- Use `describe` and `it` for structure
- Mock external dependencies, not internal modules
- Test error paths in addition to happy paths

## Release and Commit Conventions
- Bump the **minor** version for backward-compatible feature additions or meaningful capability expansions
- Bump the **major** version for breaking CLI, API, storage, or workflow changes
- Use the **patch** version for fixes, small internal improvements, and documentation-only changes
- Keep commit messages in Conventional Commits format, e.g. `feat(mission): add retry reason support`
- Prefer `feat` for user-visible functionality, `fix` for bug fixes, `refactor` for internal restructuring, and `test` for test-only changes
