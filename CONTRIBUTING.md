# Contributing to Maestro

Maestro is a local-first conductor for multi-agent software engineering,
implemented as a single Rust crate. The durable contract is repo-local files:
Harness, Task, Feature, Decision, Run, Proof, Install, and Update behavior comes
from explicit artifacts on disk, not hidden service state. This document covers
what you need to know to make a contribution.

## Quick start

```bash
git clone https://github.com/ReinaMacCredy/maestro.git
cd maestro
cargo build
./target/debug/maestro --version
cargo test
```

`maestro --version`/`-V` prints the git-derived version string; `maestro version`
adds the release date and the running binary's path.

## Repository layout

```text
maestro/
├── src/                # Rust crate: domain, operations, interfaces, foundation
├── tests/              # contract, adapter, runtime-flow, and safety tests
├── embedded/           # shipped Harness, hook, shell, and skill resources
├── .maestro/           # repo-local harness/task/run artifacts for this checkout
└── .claude/            # local Claude workflow and skill assets
```

- `src/domain/` owns the durable concepts (task, feature, proof, run, install,
  skills, extraction, harness). Domain rules live here.
- `src/operations/` holds cross-domain workflows: init, task verify, update,
  sync, harness.
- `src/interfaces/` is adapters only (cli, mcp, hooks, shell, tui). Adapters must
  not hold domain rules.
- `src/foundation/core/` is the cross-cutting safety surface: paths, schema
  constants, safe writes, backups, managed blocks, hashes.

`AGENTS.md` is the canonical contributor guide and lists every convention, with a
per-subsystem `AGENTS.md` in each major directory. Read the root one and the one
nearest your change before opening a substantive PR. See `MIGRATE.md` for the
TypeScript-to-Rust mapping, and `TESTING.md` / `MAINTENANCE.md` for the smallest
falsifying checks and the refactor/handoff discipline.

## Conventions

- Rust edition 2024. `rustfmt` is canonical; keep 4-space indentation and the
  rustfmt default line width unless an existing file shows a local exception.
- Library/domain code returns `Result` for recoverable failures. Do not panic on
  user input, path state, schema mismatch, or external command output. Use `?`
  for propagation; `expect("invariant: ...")` only for trusted internal
  invariants; `.unwrap()` only in tests/examples.
- Borrow by default: take `&str`/`&[T]` rather than `&String`/`&Vec<T>` unless
  ownership is part of the contract. Public types should implement `Debug`.
- `Cargo.toml` denies Rust warnings and Clippy `all`. Do not silence diagnostics
  with `#[allow(...)]` unless the reason is local, documented, and narrower than
  the warning.
- Keep production imports on the target facades: `crate::domain::*`,
  `crate::operations::*`, `crate::interfaces::*`, and `crate::foundation::core`.
  `crate::core`, `crate::hooks`, `crate::mcp`, `crate::shell`, and `crate::tui`
  are transitional shims, not new homes for code.
- Do not add ports/adapters/use-case stacks unless there are real alternate
  adapters or a proven test seam. Do not duplicate Harness protocol into install
  mirrors or Claude shims; shims point at the sibling `AGENTS.md`.
- Generated or installed agent-facing files are user-owned once written. A
  template refresh needs an explicit mutation path, a visible diff, a backup, or
  a force/apply story; never silently mutate existing Harness files.
- Commits follow Conventional Commits: `feat(scope):`, `fix(scope):`,
  `refactor(scope):`, `chore(scope):`. One logical change per commit.
- The version is git-derived at build time
  (`<major>.<minor>.<patch>.<commit-epoch>-g<short-sha>`, computed by `build.rs`).
  Step the version line by bumping `version` in `Cargo.toml`; releases publish
  only on manual `workflow_dispatch`.
- When editing `embedded/skills/<name>/`, `embedded/hooks/record.sh`, or
  `embedded/harness/HARNESS.md`, bump its version marker and update
  `tests/resources_version_guard.rs`.

## Required checks before opening a PR

```bash
cargo check --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All four are merge gates. Do not tag, publish, push, or create GitHub releases
without explicit approval from the maintainer.

## Filing issues

When reporting a bug, include:

- `maestro --version`.
- A minimal reproduction in a fresh repo (`maestro init` in `/tmp/foo`).
- The exact command, full output, and expected vs observed behavior.

## License

Contributions are accepted under the [MIT License](LICENSE).
