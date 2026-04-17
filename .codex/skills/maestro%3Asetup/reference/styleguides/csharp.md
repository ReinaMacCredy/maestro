# C# Style Guide

## Naming
- Types/Classes: PascalCase (`MyClass`)
- Methods: PascalCase (`DoSomething()`)
- Properties: PascalCase (`FirstName`)
- Local variables/parameters: camelCase (`myVariable`)
- Private fields: `_camelCase` (`_firstName`)
- Constants: PascalCase (`MaxRetries`)
- Interfaces: `I` prefix (`IDisposable`)
- Async methods: `Async` suffix (`GetDataAsync`)

## Formatting
- Braces on new line (Allman style)
- Use `var` when type is obvious from context
- One class per file, file name matches class name
- Use `using` directives at top, sorted alphabetically

## Modern C# (10+)
- Use file-scoped namespaces: `namespace MyApp;`
- Use `record` for immutable data types
- Use pattern matching (`is`, `switch` expressions)
- Use `required` properties (C# 11)
- Use raw string literals for multi-line strings
- Use `global using` for common namespaces

## Null Safety
- Enable nullable reference types (`#nullable enable`)
- Use `?` for nullable types, `!` sparingly
- Prefer null-conditional (`?.`) and null-coalescing (`??`, `??=`)
- Validate parameters with `ArgumentNullException.ThrowIfNull()`

## Async/Await
- Use `async`/`await` throughout (avoid `.Result` or `.Wait()`)
- Return `Task` or `ValueTask`, not `void` (except event handlers)
- Use `CancellationToken` for cancellable operations
- Avoid `async void` (except event handlers)
- Use `ConfigureAwait(false)` in library code

## Error Handling
- Catch specific exceptions, not `Exception` base class
- Use custom exceptions for domain errors
- Use `try`/`finally` or `using` for cleanup
- Throw with `throw;` to preserve stack trace (not `throw ex;`)

## LINQ
- Prefer method syntax for simple queries
- Prefer query syntax for complex joins
- Use `Any()` instead of `Count() > 0`
- Materialize with `ToList()` / `ToArray()` when reusing results
- Avoid LINQ in hot paths (performance-sensitive code)

## Dependency Injection
- Register services in `Program.cs` or startup
- Use constructor injection
- Prefer `IServiceCollection` extension methods for registration
- Scoped for per-request, Singleton for stateless services, Transient for lightweight

## Testing
- Use xUnit (preferred) or NUnit
- Use `[Fact]` for single tests, `[Theory]` for parameterized
- Use FluentAssertions for readable assertions
- Mock with Moq or NSubstitute
- Follow Arrange-Act-Assert pattern
