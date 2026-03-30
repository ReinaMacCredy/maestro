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

## Compiled Binary Verification
- After `bun run build`, verify CLI changes against the fresh repo build first: `./dist/maestro --version` and then `./dist/maestro <command-under-test>`
- Do not assume `maestro` on `PATH` is the fresh build; treat `./dist/maestro` and `/Users/reinamaccredy/.local/bin/maestro` as separate artifacts
- If you need to verify the installed `maestro` command, run `command -v maestro` first and record the resolved path in your notes
- Before testing the installed `maestro` command, refresh it from `./dist/maestro` using atomic replacement with a temp file plus `mv`; do not rely on a plain in-place overwrite
- For Mission Control or other TTY smoke tests, prefer `./dist/maestro mission-control ...` unless the goal is specifically to validate the installed command on `PATH`
- Every verification summary must state which binary was exercised: `./dist/maestro` or installed `maestro` on `PATH`

## Release and Commit Conventions
- Bump the **minor** version for backward-compatible feature additions or meaningful capability expansions
- Bump the **major** version for breaking CLI, API, storage, or workflow changes
- Use the **patch** version for fixes, small internal improvements, and documentation-only changes
- Keep commit messages in Conventional Commits format, e.g. `feat(mission): add retry reason support`
- Prefer `feat` for user-visible functionality, `fix` for bug fixes, `refactor` for internal restructuring, and `test` for test-only changes
