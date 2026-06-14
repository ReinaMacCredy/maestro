# Idiomatic Rust Style Guide Summary

This document summarizes key rules and best practices for writing idiomatic
Rust, drawn from the Rust API Guidelines, `rustfmt`, and Clippy conventions.

## 1. Formatting
- **`rustfmt`:** All Rust code **must** be formatted with `rustfmt` (`cargo fmt`). This is a non-negotiable, automated standard.
- **`clippy`:** Run `cargo clippy` and address lints. Treat warnings as defects, not noise.
- **Line Length:** Default `rustfmt` wraps at 100 columns; let the formatter decide.

## 2. Naming
- **Casing:** `snake_case` for functions, methods, variables, modules, and crates; `UpperCamelCase` for types, traits, and enum variants; `SCREAMING_SNAKE_CASE` for constants and statics.
- **Conversions:** Follow the `as_`/`to_`/`into_` convention: `as_` is a cheap borrow-to-borrow view, `to_` is an expensive clone-producing conversion, `into_` consumes `self`.
- **Getters:** Name a getter after the field (`fn owner(&self)`), not with a `get_` prefix. Use `get_` only when there is also a setter or a fallible variant.
- **Iterators:** Methods returning iterators are named `iter`, `iter_mut`, `into_iter`.

## 3. Ownership and Borrowing
- **Borrow over clone:** Accept `&str`/`&[T]` parameters rather than `String`/`Vec<T>` when you only read. Reach for `.clone()` only when ownership is genuinely required, not to silence the borrow checker.
- **Lifetimes:** Prefer elided lifetimes; name them only when the signature is ambiguous. Do not over-annotate.
- **`Copy`:** Derive `Copy` only for small, plain-data types where bitwise copy is the right semantics.

## 4. Error Handling
- **`Result` over panics:** Return `Result<T, E>` for recoverable failures. Reserve `panic!`, `unwrap`, and `expect` for genuinely unreachable states or tests; in library code they are a code smell.
- **`?` operator:** Propagate errors with `?` instead of manual `match` ladders.
- **Error types:** Implement `std::error::Error` (or use `thiserror`) for library errors; `anyhow` is fine for application-level error flow. Fail loud and early with actionable context.
- **No silent discards:** Do not ignore a `Result` with `let _ =` unless the discard is deliberate and commented.

## 5. Types and Data Modeling
- **Make illegal states unrepresentable:** Use enums and the newtype pattern to encode invariants in the type system instead of validating at runtime.
- **`Option` over sentinels:** Use `Option<T>` rather than null-like sentinel values.
- **Derive freely:** Derive `Debug` on public types; derive `Clone`, `PartialEq`, `Eq`, `Hash` where the semantics fit.
- **Newtypes:** Wrap primitive types (`struct UserId(u64)`) to prevent mixing semantically different values.

## 6. Traits and Generics
- **Standard traits:** Prefer implementing standard traits (`From`/`TryFrom`, `Default`, `Display`, `Iterator`) over inventing bespoke conversion methods.
- **`From` not `Into`:** Implement `From`; the blanket impl gives you `Into` for free.
- **Bounds:** Keep trait bounds on the `impl` or function, not the struct definition, unless the struct truly needs them.
- **Avoid premature generics:** Do not introduce a generic parameter or trait for a single concrete caller.

## 7. Modules and Visibility
- **Default to private:** Expose the minimum surface. Use `pub(crate)` for cross-module-internal items; reserve `pub` for the real API.
- **Re-exports:** Curate the public API with `pub use` in the crate root or a facade module; do not leak internal module structure to callers.
- **Flat over deep:** Prefer shallow module trees that mirror the domain.

## 8. Idioms
- **Iterators over index loops:** Prefer iterator adaptors (`map`, `filter`, `collect`) to manual indexing.
- **Pattern matching:** Use `match`, `if let`, and `let ... else` to handle enums exhaustively; avoid `unwrap` on a known-`Some`.
- **Builders:** Use the builder pattern for types with many optional fields rather than many constructors.
- **`impl Trait`:** Return `impl Trait` for unnameable iterator/closure types instead of boxing.

## 9. Comments and Documentation
- **Doc comments:** Document public items with `///`; document modules/crates with `//!`. Explain *why*, and include runnable examples where they clarify usage.
- **Sparse inline comments:** Comment the non-obvious why, not the what. A clearer name beats a comment.

## 10. Testing
- **Co-located unit tests:** Put unit tests in a `#[cfg(test)] mod tests` block in the same file; put integration tests under `tests/`.
- **Behavior over implementation:** Test through public interfaces so tests survive refactors.

*Sources: [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), [The Rust Programming Language](https://doc.rust-lang.org/book/), [rustfmt](https://github.com/rust-lang/rustfmt) and [Clippy](https://github.com/rust-lang/rust-clippy) conventions.*
