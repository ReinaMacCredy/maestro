# TypeScript Style Guide

## Types
- Prefer `interface` for object shapes, `type` for unions/intersections
- Avoid `any` -- use `unknown` and narrow with type guards
- Use `readonly` for immutable data
- Prefer `const` assertions for literal types
- Use discriminated unions over optional fields for variant types

## Naming
- Types/Interfaces: PascalCase
- Variables/Functions: camelCase
- Constants: UPPER_SNAKE_CASE
- Enums: PascalCase (members too)
- Files: kebab-case (e.g., `user-service.ts`)

## Functions
- Prefer arrow functions for callbacks and short expressions
- Use named functions for top-level declarations (better stack traces)
- Explicit return types for public API functions
- Use function overloads sparingly -- prefer union types

## Async
- Always `await` promises -- never fire-and-forget
- Use `Promise.all()` for parallel independent operations
- Handle errors with try/catch at the boundary, not every call
- Prefer `async/await` over `.then()` chains

## Imports
- Group: built-in > external > internal > relative
- Use named imports, not `import *`
- Avoid circular dependencies

## Nullability
- Prefer `undefined` over `null` (align with TS defaults)
- Use optional chaining (`?.`) and nullish coalescing (`??`)
- Avoid non-null assertions (`!`) except in tests

## Testing
- Use `describe`/`it` for structure
- Mock external dependencies, not internal modules
- Test error paths, not just happy paths
