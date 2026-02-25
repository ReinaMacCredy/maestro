# Dart Style Guide

## Formatting
- Use `dart format` (non-negotiable)
- Max line length: 80 characters
- Follow Effective Dart guidelines

## Naming
- Classes/Enums/Typedefs: PascalCase (`MyWidget`)
- Variables/Functions/Parameters: camelCase (`myVariable`)
- Constants: camelCase (`defaultTimeout`) -- NOT UPPER_SNAKE_CASE
- Libraries/Packages: snake_case (`my_package`)
- Private: prefix with underscore (`_privateField`)
- File names: snake_case (`my_widget.dart`)

## Types
- Prefer type inference with `var` for local variables
- Annotate public APIs with explicit types
- Use `final` for variables that won't be reassigned
- Use `const` for compile-time constants
- Prefer `Iterable` over `List` in parameters when possible

## Null Safety
- Sound null safety enabled (Dart 2.12+)
- Use `?` for nullable, default to non-nullable
- Use `late` sparingly (prefer nullable + null check)
- Use `!` only when you're certain (document why)
- Prefer `??` and `?.` operators

## Classes
- Use named constructors for clarity: `MyClass.fromJson()`
- Use factory constructors for caching or returning subtypes
- Prefer `final` fields with constructor initialization
- Use `@override` annotation
- Prefer `mixin` over abstract class for shared behavior

## Async
- Use `async`/`await` (not `.then()` chains)
- Return `Future<T>` from async functions
- Use `Stream` for multiple async values
- Cancel subscriptions in `dispose()`
- Use `Completer` for wrapping callback APIs

## Flutter-Specific
- Keep widgets small and focused
- Extract widget methods into separate widget classes
- Use `const` constructors where possible
- Prefer `StatelessWidget` over `StatefulWidget`
- Use `ValueNotifier` + `ValueListenableBuilder` for simple state
- Use `Key` for list items and animated widgets

## Error Handling
- Catch specific exceptions: `on FormatException catch (e)`
- Use `rethrow` to preserve stack trace
- Custom exceptions extend `Exception` (not `Error`)
- Use `Result` pattern for expected failures

## Testing
- Use `test` package for unit tests
- Use `flutter_test` for widget tests
- Use `mockito` or `mocktail` for mocking
- Structure: `group()` + `test()` or `testWidgets()`
- Use `setUp()` and `tearDown()` for fixtures
- Golden tests for UI regression detection
