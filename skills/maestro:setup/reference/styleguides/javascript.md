# JavaScript Style Guide

## Basics
- Use `const` by default, `let` when reassignment needed, never `var`
- Use strict equality (`===`) always
- Use template literals over string concatenation
- Prefer destructuring for object/array access

## Naming
- Variables/Functions: camelCase
- Classes: PascalCase
- Constants: UPPER_SNAKE_CASE
- Files: kebab-case
- Boolean variables: prefix with `is`, `has`, `should`

## Functions
- Prefer arrow functions for callbacks
- Use named function declarations for top-level (hoisting + stack traces)
- Default parameters over `||` fallbacks
- Rest parameters over `arguments` object

## Async
- `async/await` over `.then()` chains
- Always handle promise rejections
- Use `Promise.all()` for parallel work
- Use `AbortController` for cancellable operations

## Modules
- ES modules (`import`/`export`) over CommonJS (`require`)
- Named exports over default exports (better refactoring)
- One module per file
- Index files only for re-exports

## Error Handling
- Throw `Error` objects, not strings
- Custom error classes extend `Error`
- Try/catch at boundaries, propagate elsewhere
- Validate external input at entry points

## Arrays & Objects
- Use array methods (`map`, `filter`, `reduce`) over loops
- Spread syntax for shallow copies
- `Object.keys()`/`Object.entries()` for iteration
- Optional chaining (`?.`) and nullish coalescing (`??`)

## Testing
- Describe/it structure
- One concept per test
- Mock external dependencies (fetch, filesystem, timers)
- Test error scenarios
