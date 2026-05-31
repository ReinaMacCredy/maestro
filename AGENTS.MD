
## STRUCTURE

```text
maestro/
├── src/                 # Rust crate: domain, operations, interfaces, foundation
├── tests/               # contract, adapter, runtime-flow, and safety tests
├── embedded/           # shipped Harness, hook, shell, and skill resources
├── .maestro/            # repo-local harness/task/run artifacts for this checkout
├── .claude/             # local Claude workflow and skill assets
├── ARCHITECTURE.md      # module ownership and target architecture
├── TESTING.md           # smallest falsifying checks by touched surface
└── MAINTENANCE.md       # refactor discipline, drift rules, handoff standard
```

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Add or change CLI verbs | `src/interfaces/cli/` | Adapter only; domain rules stay behind owning facades. |
| Change task lifecycle or artifacts | `src/domain/task/` | Preserve optimistic concurrency, state history, blockers, acceptance lock. |
| Change verification behavior | `src/domain/proof/`, `src/operations/task_verify/` | Proof writes reports; Task applies lifecycle outcomes. |
| Change hook/event capture | `src/domain/run/`, `src/interfaces/hooks/` | Preserve normalized append, partial-line tolerance, symlink rejection. |
| Change install mirrors or local agent files | `src/domain/install/`, `embedded/`, root agent files | Do not duplicate Harness protocol into mirrors. |
| Change update/release logic | `src/operations/update/`, `src/interfaces/cli/update.rs` | Passive checks must not mutate user-owned Harness files. |
| Change shared path/schema/write helpers | `src/foundation/core/` | Treat as cross-cutting safety surface. |
| Choose verification | `TESTING.md`, `tests/` | Start with the owning module row, then broaden only for public contracts. |

## CODE MAP

| Symbol | Type | Location | Role |
| --- | --- | --- | --- |
| `main` | fn | `src/main.rs` | Parse CLI, run command, trigger passive auto-check except excluded commands. |
| `Cli` | struct | `src/interfaces/cli/mod.rs` | Root clap parser for the binary. |
| `RootCommand` | enum | `src/interfaces/cli/mod.rs` | Public CLI command surface. |
| `domain` | module | `src/domain/mod.rs` | Durable concepts: Harness, Task, Feature, Decision, Run, Proof, Install. |
| `operations` | module | `src/operations/mod.rs` | Cross-domain workflows: init, task verify, update, migrate, metrics, improve. |
| `foundation::core` | module | `src/foundation/core/` | Paths, schema constants, safe writes, backups, managed blocks, hashes. |

## CONVENTIONS

- Local-first correctness wins over convenience. Commands derive behavior from
  files on disk, not hidden daemon state.
- Start code navigation with `srcwalk guide` once, then `srcwalk overview`,
  `discover`, `context`, `show`, `trace`, or `deps`. Use `rg` for raw text
  confirmation after structural navigation.
- Keep new production imports on target facades:
  `crate::domain::*`, `crate::operations::*`, `crate::interfaces::*`, and
  `crate::foundation::core`.
- Compatibility roots such as `crate::core`, `crate::hooks`,
  `crate::mcp`, `crate::shell`, and `crate::tui` are transitional shims.
- Generated or installed agent-facing files are user-owned once written.
  Template refresh needs an explicit mutation path, visible diff, backup, or
  force/apply story.

## ANTI-PATTERNS (THIS PROJECT)

- Do not silently mutate existing Harness files from `install`, passive update
  checks, or ordinary binary/resource updates.
- Do not put domain rules in CLI, MCP, hook, shell, or TUI adapters.
- Do not add broad ports/adapters/use-case stacks unless there are real
  alternate adapters or a proven test seam.
- Do not duplicate long Harness or AGENTS content into Claude shims. Shims point
  at the sibling `AGENTS.md`.
- Do not touch unrelated dirty files in this checkout.

## COMMANDS

```bash
git status --short --branch
git log -1 --oneline
cargo check --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check -- ARCHITECTURE.md TESTING.md MAINTENANCE.md AGENTS.md
```

Release verification uses the stricter release contract:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
target/debug/maestro --version
```

Do not tag, publish, push, or create GitHub releases without explicit approval.

## NOTES

- Version source is `Cargo.toml` (`package.version`); CLI versions render as
  pure semver via `env!("CARGO_PKG_VERSION")`. Repo-tracked behavior changes
  bump the version; docs-only do not. Release publishing requires manual
  dispatch or a head commit named `chore(release): v<version>`.
- If editing `embedded/skills/<name>/`, `embedded/hooks/record.sh`, or
  `embedded/harness/HARNESS.md`, bump its version marker and update
  `tests/resources_version_guard.rs`.
- Docs-only edits still require reading back changed files and checking
  consistency with `ARCHITECTURE.md`, `TESTING.md`, and `MAINTENANCE.md`.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- none (root)

Children:
- [.claude/AGENTS.md](.claude/AGENTS.md)
- [embedded/AGENTS.md](embedded/AGENTS.md)
- [src/AGENTS.md](src/AGENTS.md)
- [src/domain/AGENTS.md](src/domain/AGENTS.md)
- [src/domain/proof/AGENTS.md](src/domain/proof/AGENTS.md)
- [src/foundation/core/AGENTS.md](src/foundation/core/AGENTS.md)
- [src/interfaces/cli/AGENTS.md](src/interfaces/cli/AGENTS.md)
- [src/operations/AGENTS.md](src/operations/AGENTS.md)
- [tests/AGENTS.md](tests/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->

# Rust Style Guide Summary

This document summarizes key rules from the official Rust Style Guide and the Rust API Guidelines, plus AI-agent-specific discipline to avoid common LLM failure modes.

## 1. Formatting
- **`rustfmt`:** All Rust code **must** be formatted with `rustfmt`. This is the canonical, automated standard.
- **Indentation:** **4 spaces**, never tabs. All indentation must be a multiple of 4.
- **Line Width:** **100 character** maximum. Comments cap at 80 or the line width, whichever is smaller.
- **Trailing Commas:** Use in multi-line comma-separated lists.
- **Whitespace:** Zero or one blank line between items. No trailing whitespace.

## 2. Naming
- **Types, Traits, Enums:** `UpperCamelCase`.
- **Functions, Methods, Variables, Modules, Crates:** `snake_case`.
- **Constants and Statics:** `SCREAMING_SNAKE_CASE`.
- **Conversions:** `as_` (cheap reference), `to_` (expensive non-consuming), `into_` (consuming).
- **Getters:** A getter for field `owner` is named `owner()`, not `get_owner()`.
- **Iterators:** Producing methods are `iter`, `iter_mut`, `into_iter`. Iterator types match the method.

## 3. Ownership and Borrowing
- **Borrow by default:** Take `&T` or `&mut T` unless ownership is required.
- **Strings and slices:** Take `&str`, not `&String`. Take `&[T]`, not `&Vec<T>`.
- **Lifetimes:** Lean on lifetime elision; introduce named lifetimes only when semantics require them. Never return a reference to a local.

## 4. Error Handling
- **`Result<T, E>` for recoverable errors; `panic!` only for true bugs.** Library code must not panic on user input.
- **`?` operator:** Use for error propagation. Never use the obsolete `try!` macro.
- **Custom Errors:** Implement `std::error::Error`, `Display`, and `Debug`. Use `thiserror` for typed library errors and `anyhow` for application binaries.
- **Destructors:** Never fail in `Drop`. Provide an explicit `close()` when failure is possible.

## 5. Types and Traits
- **Newtypes:** Wrap primitives in a struct for static distinctions (e.g., `struct UserId(u64)`).
- **Argument types convey meaning.** Avoid raw `bool` and `Option` parameters; use enums or newtypes.
- **Common Traits:** Eagerly derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Default` where reasonable. All public types must implement `Debug`.
- **Conversions:** Implement `From`; `Into` is derived automatically. Use `AsRef` / `AsMut` for cheap reference conversions.
- **Sealed Traits:** Use a sealed supertrait to prevent downstream implementations of public traits.

## 6. Concurrency
- **`Send` + `Sync`:** Implement where reasonable so types compose across threads.
- **Cross-task communication:** Prefer `mpsc` channels; use `Arc<Mutex<T>>` when shared mutable state is the simpler model.
- **Read-heavy access:** Consider `RwLock` over `Mutex`.

## 7. Modules and Imports
- **Layout:** Group imports as standard library, external crates, then internal modules; one blank line between groups.
- **Paths:** Prefer absolute paths from the crate root. Avoid glob imports outside preludes and tests.
- **Visibility:** Default to private. Use `pub(crate)` and `pub(super)` to scope; reserve `pub` for the crate's API.

## 8. Idioms
- **Iterators over loops:** Prefer `.iter().map().filter().collect()` over index-based loops.
- **Pattern matching:** Prefer `match`, `if let`, and `let else` over nested conditionals.
- **Combinators:** Use `map`, `and_then`, `ok_or`, `unwrap_or_else` on `Option`/`Result` for short transforms.
- **`Cow<T>`:** Use for APIs that may borrow or own.

## 9. Documentation
- **Doc Comments:** Use `///` for items and `//!` for module/crate roots. Every public item gets a doc comment.
- **Examples:** Include a runnable example in rustdoc. Use `?` in examples, never `unwrap()` or `try!`.
- **Sections:** Document `# Errors`, `# Panics`, and `# Safety` when relevant.
- **Cargo.toml:** Include `description`, `license`, `repository`, `documentation`, `keywords`, and `categories`.

## 10. AI Agent Discipline

The compiler is your code review. Lean on it; do not paper over its complaints.

- **Compiler-first loop:** After every meaningful edit, run `cargo check`, then `cargo clippy --all-targets -- -D warnings`, then `cargo test`. Resolve every diagnostic.
- **No `.unwrap()` reflex.** LLM training data is full of blog snippets that `.unwrap()` for brevity. Use `?` or `expect("invariant: ...")`; `.unwrap()` only in tests and examples.
- **No `panic!` to avoid an error path.** Return `Result` and define a variant if needed.
- **No `std::sync::Mutex` across `.await`.** Use `tokio::sync::Mutex` or scope the guard so it drops before any `.await`.
- **No `.clone()` to silence the borrow checker.** Refactor or scope the borrow.
- **No undocumented `unsafe`.** Every `unsafe` block carries a `// SAFETY:` comment; every `unsafe fn` documents preconditions in a `# Safety` section.
- **Missing methods are usually missing traits.** When the compiler says a method "doesn't exist," `use` the relevant trait before calling.
- **TDD over speculation:** Write a failing test, make it pass, refactor. The test is the contract.
- **Match the existing style.** When editing a file, follow the patterns already there; do not rewrite to a different idiom.

## 11. Tooling
- **`cargo fmt`:** Run before every commit.
- **`cargo clippy --all-targets -- -D warnings`:** Treat warnings as errors. Resolve, do not silence with `#[allow(...)]` unless justified inline.
- **`cargo test`:** Unit tests in-file (`#[cfg(test)] mod tests`); integration tests under `tests/`.
- **`cargo doc --no-deps`:** Build docs in CI to catch broken intra-doc links.
