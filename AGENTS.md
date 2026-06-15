# Maestro Agent Notes

## OVERVIEW

Maestro is a local-first Rust CLI. The durable contract is repo-local files:
Harness, Task, Feature, Decision, Run, Proof, Install, and Update behavior must
come from explicit artifacts, not hidden service state.

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
| `operations` | module | `src/operations/mod.rs` | Cross-domain workflows: init, task verify, update, sync, harness. |
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
- Compatibility roots `crate::hooks`, `crate::mcp`, and `crate::tui` are
  transitional shims.
- Generated or installed agent-facing files are user-owned once written.
  Template refresh needs an explicit mutation path, visible diff, backup, or
  force/apply story.
- Write verbs echo only the computed delta the agent could not derive from its
  own flags (minted id, bindings, supersession, the appended note), plus at most
  one next-step pointer. Never re-print the agent's own input or a standing
  multi-line footer; the full record stays one read verb away (e.g.
  `maestro decision show <id>`).
- Read verbs bound their default output to the live/relevant slice so routine
  orientation does not re-ingest the whole store: `list` hides coarse-closed
  cards, `decision list`/`query decisions` show only the recent window. The full
  set is one `--all` away, and `--feature <id>` scopes decisions; the count
  header names what was hidden and how to widen.

## CONCURRENT AGENT CONTRACT

- Multiple agents may run Maestro commands in the same repo. Durable card-store
  writes use snapshot/CAS: the command writes only if the card or entry file
  still matches the bytes it read.
- A racing writer gets a retryable error containing
  `changed since it was read; re-run the command`. Re-run the command so it
  reads the newest store state and reapplies the intended change.
- Dir-backed card deletion also compares the card file snapshot before removing
  the directory and sidecars. Entry-backed card deletion rewrites the entry file
  through whole-file CAS.
- Append-only logs and notes use append-mode writes; they preserve complete
  lines but do not provide a cross-file transaction. Multi-file commands can
  interleave at file boundaries, so treat a failed command as partial unless its
  operation documents rollback.
- Agent-facing read contracts are JSON, not human text. `maestro ready --json`
  emits `version`, `schema`, and `cards` with stable `rank`, `id`, `type`,
  `title`, `status`, `parent`, and `claimed_by`. `maestro list --json` emits
  `version`, `schema`, and `cards` with stable `id`, `type`, `title`, `status`,
  `parent`, `claimed_by`, `claimed_at`, and `archived`. These fields are
  additive-only: new fields may appear, but existing meanings are not silently
  renamed or reinterpreted. In a repo without a card store the JSON verbs
  still emit a valid envelope with zero cards (exit 0); the guiding notice
  goes to stderr so stdout stays parseable.

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
target/debug/maestro version
```

After committing, always release locally so manual testing runs the newest
binary:

```bash
cargo build --release
cp target/release/maestro ~/.local/bin/maestro
maestro version   # g<short-sha> must match HEAD
```

Do not tag, publish, push, or create GitHub releases without explicit approval.

## NOTES

- Version is git-derived at build time: `<major>.<minor>.<patch>.<commit-epoch>-g<short-sha>`,
  computed by `build.rs` and injected as `env!("MAESTRO_VERSION")`. The `<major>.<minor>.<patch>`
  prefix is Cargo.toml's `version` (the commit-epoch is appended as a 4th dotted component), so
  bump Cargo.toml to step the version line. `maestro --version`/`-V` prints the version string;
  `maestro version` adds the release date and the running binary's path. Releases publish ONLY on
  manual `workflow_dispatch`; ordinary commits and merges never release.
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
- [embedded/schemas/AGENTS.md](embedded/schemas/AGENTS.md)
- [src/AGENTS.md](src/AGENTS.md)
- [src/domain/AGENTS.md](src/domain/AGENTS.md)
- [src/domain/card/AGENTS.md](src/domain/card/AGENTS.md)
- [src/domain/feature/AGENTS.md](src/domain/feature/AGENTS.md)
- [src/domain/install/AGENTS.md](src/domain/install/AGENTS.md)
- [src/domain/proof/AGENTS.md](src/domain/proof/AGENTS.md)
- [src/domain/run/AGENTS.md](src/domain/run/AGENTS.md)
- [src/domain/task/AGENTS.md](src/domain/task/AGENTS.md)
- [src/foundation/core/AGENTS.md](src/foundation/core/AGENTS.md)
- [src/interfaces/cli/AGENTS.md](src/interfaces/cli/AGENTS.md)
- [src/operations/AGENTS.md](src/operations/AGENTS.md)
- [tests/AGENTS.md](tests/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->

## RUST STYLE

- `rustfmt` is canonical. Keep Rust indentation at 4 spaces and line width near
  the rustfmt default unless an existing file shows a local exception.
- `Cargo.toml` denies Rust warnings and Clippy `all`; do not silence diagnostics
  with `#[allow(...)]` unless the reason is local, documented, and narrower than
  the warning.
- Library/domain code returns `Result` for recoverable failures. Do not panic on
  user input, path state, schema mismatch, or external command output.
- Use `?` for propagation. Use `expect("invariant: ...")` only for trusted
  internal invariants; `.unwrap()` belongs only in tests/examples.
- Borrow by default: take `&str`/`&[T]` rather than `&String`/`&Vec<T>` unless
  ownership or allocation is part of the contract.
- Public types should implement `Debug`; derive common traits when they help
  tests or stable data contracts.
- Keep module facades small and intentional. Do not expose child modules or add
  new abstraction layers unless the architecture docs already define the seam.
