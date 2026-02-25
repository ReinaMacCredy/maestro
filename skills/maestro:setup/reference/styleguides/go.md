# Go Style Guide

## Formatting
- Use `gofmt` / `goimports` (non-negotiable)
- Follow Effective Go and Go Code Review Comments

## Naming
- Exported: PascalCase; unexported: camelCase
- Short variable names for small scopes (`i`, `r`, `ctx`)
- Interfaces: verb suffix `-er` (e.g., `Reader`, `Writer`, `Stringer`)
- Avoid `Get` prefix for getters (use `Name()` not `GetName()`)
- Package names: short, lowercase, no underscores

## Error Handling
- Always check errors -- never use `_` for error returns
- Wrap errors with context: `fmt.Errorf("doing X: %w", err)`
- Use sentinel errors (`var ErrNotFound = errors.New(...)`) for expected conditions
- Use custom error types for structured error data
- Return errors, don't panic (except truly unrecoverable)

## Concurrency
- Prefer channels for communication, mutexes for state
- Never start goroutines without a way to stop them
- Use `context.Context` for cancellation and timeouts
- Use `sync.WaitGroup` for goroutine lifecycle
- Use `errgroup` for parallel operations with error handling

## Interfaces
- Accept interfaces, return structs
- Keep interfaces small (1-3 methods)
- Define interfaces at the consumer, not the producer
- Use `io.Reader`/`io.Writer` when possible

## Packages
- One package per directory
- Avoid circular dependencies (use interfaces to break cycles)
- Internal packages for private implementation
- `cmd/` for entry points, `internal/` for private code

## Testing
- Table-driven tests with `t.Run()`
- Use `testing.T` helpers, not assertion libraries
- `testdata/` for test fixtures
- Use `t.Helper()` in test utility functions
- Benchmarks with `testing.B` for performance-sensitive code
