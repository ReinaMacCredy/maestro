# C++ Style Guide

## Formatting
- Follow Google C++ Style Guide or project `.clang-format`
- Max line length: 80 characters (Google) or project standard
- Use `clang-format` for automatic formatting

## Naming
- Types/Classes: PascalCase (`MyClass`)
- Functions: PascalCase (`DoSomething()`) or camelCase (project-dependent)
- Variables: snake_case (`my_variable`)
- Constants: `kPascalCase` or `UPPER_SNAKE_CASE`
- Member variables: trailing underscore (`member_var_`)
- Namespaces: lowercase (`my_namespace`)
- Macros: `UPPER_SNAKE_CASE` (avoid macros when possible)

## Memory Management
- Prefer smart pointers (`std::unique_ptr`, `std::shared_ptr`) over raw pointers
- Use `std::make_unique` and `std::make_shared`
- Follow RAII (Resource Acquisition Is Initialization)
- Avoid `new`/`delete` in application code
- Use `std::move` for transferring ownership

## Modern C++ (C++17/20)
- Use `auto` for complex types, explicit types for clarity
- Prefer `std::optional` over sentinel values
- Use structured bindings: `auto [key, value] = pair;`
- Use `constexpr` for compile-time computation
- Prefer `std::string_view` over `const std::string&` for read-only access
- Use `[[nodiscard]]` for functions where ignoring return value is a bug

## Error Handling
- Use exceptions for truly exceptional conditions
- Use `std::expected` (C++23) or error codes for expected failures
- Never throw in destructors
- Catch by const reference: `catch (const std::exception& e)`

## Headers
- Include guards: `#pragma once` or traditional `#ifndef` guards
- Include order: related header, C system, C++ standard, third-party, project
- Forward-declare when possible to reduce compile times
- Avoid includes in headers when forward declarations suffice

## Classes
- Use `explicit` on single-argument constructors
- Follow Rule of Five (or Rule of Zero)
- Prefer composition over inheritance
- Mark virtual destructors as `virtual`
- Use `override` keyword on all overriding methods
- Use `final` on classes/methods that should not be further overridden

## Testing
- Use Google Test or Catch2
- One test file per source file
- Use test fixtures for shared setup
- `EXPECT_*` for non-fatal, `ASSERT_*` for fatal assertions
- Use death tests for crash/abort verification
