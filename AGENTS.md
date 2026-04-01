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
- For user-facing CLI or TUI work, finish by running `bun run release:local` so the local `maestro` command on `PATH` is refreshed to the newest compiled build before sign-off
- When reviewing the Maestro TUI, start with `./dist/maestro mission-control --once` to smoke-test a single read-only frame before doing interactive TTY validation
- If you need to verify the installed `maestro` command, run `command -v maestro` first and record the resolved path in your notes
- Before testing the installed `maestro` command, refresh it from `./dist/maestro` using atomic replacement with a temp file plus `mv`; do not rely on a plain in-place overwrite
- After `bun run release:local`, verify both `maestro --version` and `./dist/maestro --version`, and record the installed path from `command -v maestro`
- For Mission Control or other TTY smoke tests, prefer `./dist/maestro mission-control ...` unless the goal is specifically to validate the installed command on `PATH`
- Every verification summary must state which binary was exercised: `./dist/maestro` or installed `maestro` on `PATH`

## Mission Control Contracts
- Keep `buildSnapshot()` and `buildHomeSnapshot()` read-only; do not perform runtime recovery, feature updates, or other state mutation inside snapshot projection
- `mission-control --json` and `mission-control --once` must remain read-only inspection paths; recovery or supervision belongs only in explicit orchestration/supervised runtime paths
- When adding Mission Control tests, cover both source-run and compiled `./dist/maestro` behavior if the change affects interactive flow, polling, or TTY handling

## Shell Gotchas
- When running `git commit -m ...` through `zsh -lc`, do not put Markdown backticks inside double-quoted commit messages; use single-quoted heredocs, a temp file, or escaped backticks to avoid accidental command substitution

## Environment-Sensitive Tests
- Treat `tests/integration/session-sourcepath.test.ts` as environment-dependent: if `sourcePath` existence assertions fail, verify the expected local Claude session artifact exists before blaming unrelated code changes

## Release and Commit Conventions
- Bump the Maestro CLI version for every repo-tracked code change that affects runtime behavior, CLI output, TUI behavior, storage behavior, or user-visible workflows so the running binary can be identified exactly after each change
- Treat documentation-only or comment-only changes as exempt from version bumps unless they ship alongside behavior changes
- Make the version bump part of the same working increment and commit as the behavior change; do not leave version updates for a later cleanup commit
- `bun scripts/auto-bump.ts` computes the next version from conventional commits and updates tracked version files, but does not build, tag, install, or publish by itself
- `bun scripts/ci.ts` is the full local release flow: auto-bump, test, build, commit the release, tag it, and install the local binary
- `bun run release:local` only rebuilds and reinstalls the local `maestro` binary; it does not bump the version or create a release commit
- `bun run deploy` currently uses the manual bump flow (`bun run bump`) rather than `auto-bump`; do not assume `deploy` applies conventional-commit versioning unless it is updated explicitly
- Bump the **minor** version for backward-compatible feature additions or meaningful capability expansions
- Bump the **major** version for breaking CLI, API, storage, or workflow changes
- Use the **patch** version for fixes, small internal improvements, and documentation-only changes
- Keep commit messages in Conventional Commits format, e.g. `feat(mission): add retry reason support`
- Prefer `feat` for user-visible functionality, `fix` for bug fixes, `refactor` for internal restructuring, and `test` for test-only changes
