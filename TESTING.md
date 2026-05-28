# Maestro Testing Guide

This guide expands the Testing Map in `ARCHITECTURE.md`. Use
`ARCHITECTURE.md` to understand module ownership; use this file to choose the
smallest checks that can falsify a change.

## Default Verification

For docs-only changes:

```sh
git diff --check -- ARCHITECTURE.md TESTING.md MAINTENANCE.md
```

Also read back the changed section and check that it does not contradict
`ARCHITECTURE.md`, `TESTING.md`, or `MAINTENANCE.md`. If the change touches
repo-local agent instructions, include the tracked agent file path for this
checkout, such as `AGENTS.md` or `AGENTS.MD`.

For Rust code changes, start with the compiler:

```sh
cargo check --all-targets
```

Before handoff for a meaningful code change, run:

```sh
cargo check --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

When public Rust docs, public item comments, or intra-doc links change, also
run:

```sh
cargo doc --no-deps
```

## Targeted Test Commands

Run one integration test file:

```sh
cargo test --test task_verify_integration
```

Run one integration test by name:

```sh
cargo test --test task_verify_integration task_verify_passes_with_event_proof_and_persists_verification_json
```

Run tests matching a filter:

```sh
cargo test install
```

Use targeted tests during the edit loop, then broaden when the changed contract
has callers or user data safety impact.

## Test Types

| Type | Purpose | When to use |
| --- | --- | --- |
| Contract tests | Prove one domain module's public behavior and invariants. | First choice for private implementation changes inside a module. |
| Adapter tests | Prove CLI, MCP, hook, shell, or TUI surfaces call domain contracts and render expected output. | Use when parsing, output, command routing, or non-CLI interface behavior changes. |
| Runtime-flow tests | Prove multiple module contracts compose in an important workflow. | Use when changing a public contract, artifact layout, or cross-module behavior. |
| Safety tests | Prove path containment, symlink rejection, rollback, backups, no partial writes, schema mismatch handling, and optimistic concurrency. | Required when user files, install state, migration, update, or managed writes are touched. |
| Read-model tests | Prove projections read source artifacts correctly without mutating them or depending on private write details. | Use when Metrics, Query, MCP, TUI, Watch, or status projections change. |

## Module Test Matrix

| Surface | Primary tests | Broaden to |
| --- | --- | --- |
| Foundation/Core | `tests/core_paths_fs.rs`, `tests/core_schema_error.rs`, `tests/core_managed_blocks.rs`, `tests/core_backup_diff_git.rs` | Any module whose safety policy uses the changed Core helper. |
| Harness | `tests/harness_templates.rs`, `tests/init_integration.rs`, `tests/harness_backlog.rs` | `tests/install_mirrors.rs`, `tests/update_integration.rs`, and Proof tests when Harness verification config changes, backlog proposal refresh changes, or explicit Harness apply behavior changes. |
| Task | `tests/task_lifecycle.rs`, `tests/task_artifacts.rs`, `tests/task_commands_integration.rs` | `tests/task_verify_integration.rs`, Query, TUI, MCP, and Metrics tests when task state, verification binding, or layout changes. |
| Feature | `tests/feature_decision_artifacts.rs`, `tests/feature_decision_commands_integration.rs` | Query, Doctor, TUI, and MCP tests when feature read models or output change. |
| Decision | `tests/feature_decision_artifacts.rs`, `tests/feature_decision_commands_integration.rs` | Query, docs, schema-constant, and migration tests when decision layout or metadata changes. |
| Run | `tests/hook_record_integration.rs`, `tests/run_evidence_integration.rs` | `tests/task_verify_integration.rs` and `tests/metrics_improve_integration.rs` when event proof, concurrency, evidence regeneration, or metrics reads change. |
| Proof | `tests/task_verify_integration.rs` | Task, Query, TUI, MCP, and Run tests when verification status, freshness, report writing, unapplied reports, or binding behavior changes. Include Proof-to-Task transaction tests for stale snapshots and partial failure. |
| Install | `tests/install_mirrors.rs`, `tests/install_uninstall_integration.rs`, `tests/skills_symlink_integration.rs` | Harness, Skills, and safety tests when managed blocks, hooks, locks, symlinks, transaction state, or recovery behavior change. |
| Skills | `tests/skills_extract.rs`, `tests/skills_symlink_integration.rs` | Install and Update tests when extraction rollback, recursive skill resources, executable metadata, or installed skill wiring changes. |
| Migration | `tests/migrate_integration.rs` | Target domain loader or contract tests for every artifact Migration writes; direct-write exceptions need explicit fixture coverage. |
| Update | `tests/update_integration.rs`, module-local tests in `src/operations/update/mod.rs` and `src/interfaces/cli/update.rs` | Skills extraction tests, Harness non-mutation tests, and schema/migration tests when update touches bundled skills, compatibility checks, or schema drift reporting. |
| Metrics, Improve, MCP | `tests/metrics_improve_integration.rs` | Task, Run, Proof, Feature, and Harness Backlog contract tests when source read models or backlog proposal refresh changes. |
| Shell | `tests/shell_init_integration.rs` | Install tests if shell output starts depending on install state. |
| TUI and Watch | Module-local tests in `src/interfaces/tui/task_list_watch.rs` plus command/read-model tests for the source data | Task, Feature, Proof, and Run tests when displayed fields or freshness logic change. |
| CLI surface | `tests/cli_help.rs` and the command-specific integration test | The owning domain contract tests when CLI behavior encodes domain rules. |
| Architecture/import boundaries | `tests/architecture_imports.rs` | Any moved module, compatibility alias removal, facade-protected import rule, source-layout refactor, Task/Proof/Run contract-edge change, or Install/Migration/Update/Init/Improver/Metrics safety-boundary change. |
| Template and resource content | Owning module tests for Harness, Skills, Shell, Decision, Task, or Install | At least one command integration test that writes or renders the resource. |
| End-to-end demos | `tests/phase3_core_verbs_e2e.rs`, `tests/v1_demo.rs` | Use after broad architecture, schema, or workflow changes. |

`tests/support.rs` is shared test support, not a standalone suite.

## Change Selection Rules

If changing private code inside one module, run that module's contract tests.
Then run `cargo check --all-targets`.

If reorganizing private files behind an existing module facade, run that
module's contract tests, the owning module or integration test slice, and
`cargo check --all-targets`. Use `cargo check --all-targets --all-features`
when features are relevant. Run doctests when facade docs expose import paths.
Run snapshot or golden tests when user-visible output, error text, path text, or
debug formatting can change. Broaden further when the facade exports, adapter
behavior, schema, path layout, or durable artifact behavior changed.

If adding role-grouped module roots, moving code between `interfaces/`,
`domain/`, `operations/`, or `foundation/`, or removing a legacy compatibility
path, run `cargo test --test architecture_imports`.

If adding or changing a module facade, verify that approved import paths still
work and deep implementation imports do not leak into adapters or unrelated
modules. Once a facade policy is accepted for a module, add or update a
lightweight architecture/import test for that module.
When a move splits legacy compatibility roots from target `domain::*` facades,
pin both surfaces separately: legacy roots keep old public imports, while target
facades expose only the approved contract.

If changing a module's public contract, run that module's contract tests plus
adapter and runtime-flow tests for every caller.

If changing schema versions, path layout, managed writes, backups, rollback,
or symlink policy, run the owning module tests plus the relevant safety,
migration, install, or update tests.

If changing an allowed contract edge between modules, run contract tests for
both modules and at least one runtime-flow or operation test that exercises the
edge.

If changing the Task, Proof, or Run boundary, update and run
`tests/architecture_imports.rs`. The guardrails should keep Task verification
application behind `operations/task_verify`, keep Proof on Run read-model
symbols instead of unmanaged path readers, and keep Run from importing Task
while still allowing Run event/read models to expose opaque `task_id` strings.

If changing the Install, Migration, Update, Init, Improver, or Metrics safety
boundaries, run `tests/architecture_imports.rs`. The
`operations_do_not_depend_on_interfaces` and
`domain_does_not_depend_on_interfaces_or_operations` guards keep the dependency
direction intact across Init, Improver, and Metrics; the
`install_production_sources_use_domain_facade_not_legacy_shim` guard keeps
Install as the only domain-owned orchestration exception; and the
`update_routes_schema_drift_through_migration_and_does_not_import_harness_writes`
guard keeps Update routing schema drift through the `operations/migrate` root
facade instead of importing the Harness template write surface. These are
import-boundary checks; Migration and Update behavioral coverage still lives in
`tests/migrate_integration.rs` and `tests/update_integration.rs` per the rows
above.

If changing current Task verification surfaces such as
`src/domain/proof/verify_task.rs`, the legacy `src/verification` shim, the task
verify command path, `operations/task_verify`, Proof report writing, or Task
verification binding behavior, run `tests/task_verify_integration.rs`, Task
lifecycle tests, and focused tests for stale Task snapshots, report-write
failure, typed Task-apply failure leaving a readable unapplied report, failed
verification moving a previously verified Task only through Task-owned
lifecycle logic, concurrent verifies not overwriting the canonical applied
report with a stale attempt, Harness verify commands not replacing
`acceptance.yaml` with a symlink, symlinked `verification.attempts/` read/write
rejection, symlinked canonical `verification.json` rejection, latest-attempt
marker maintenance, rollback-safe canonical report promotion when the final
Task write fails, interrupted canonical-promotion recovery through the restore
journal, and Proof status deriving applied/unapplied state from the Task-owned
applied-report receipt instead of state-history text.

If changing Run append, event read models, or run evidence generation, run hook
and run evidence tests plus focused tests for concurrent same-session appends,
Stop racing with late events, idempotent evidence regeneration, partial JSONL
line tolerance, and symlinked run path handling.

If changing Install locks, managed mirrors, or uninstall ownership behavior, run
Install and Skills tests plus focused interrupted-install recovery tests: after
lock save, after partial mirror writes, reinstall recovery, and uninstall with
unverifiable ownership, persisted removing-state retry, verified JSON restore
metadata, and install-before-init Harness prerequisite behavior.

If changing Harness templates for existing repositories, run tests proving
passive update/check paths do not mutate Harness files. Run explicit Harness
apply tests that prove diff, backup, and force/apply intent before any
user-owned Harness mutation.

If changing Harness Backlog proposal refresh, run `tests/harness_backlog.rs` for
schema validation, duplicate handling, deterministic ordering,
refresh-without-apply, and read-model behavior.

If activating a reserved schema constant such as `maestro.decision.v1` or
`maestro.run.v1`, add artifact format, migration, reader, and schema tests in
the same change. If it stays reserved, tests should not require the missing
artifact.

If changing CLI output only, run `tests/cli_help.rs` when help text changes and
the command-specific integration test when command output changes.

If changing a read model, run the read-model test plus at least one flow that
produces the source artifact it reads.

If changing generated or installed agent-facing files, run Install or Init tests
and read the generated artifact text in the failing or touched test fixture.

If changing editable resource content, run the owning module tests plus at least
one command integration test that writes, extracts, installs, or renders the
resource.

If changing bundled skills, run Skills tests and any Install or Update tests
that prove recursive extraction, symlink wiring, executable metadata, and
rollback behavior.

If changing bundled skill subfolders such as `references/`, `scripts/`, or
`assets/`, prove the full directory is installed under
`.maestro/skills/<skill-name>/` and that development-only `evals/` content is
not installed unless explicitly intended.

If changing Migration, run `tests/migrate_integration.rs` and the target domain
tests for artifacts Migration creates. Migration's current direct-write
exceptions are Task artifacts, Feature registry, Decision markdown, Harness
config, Run logs under `runs/migrated/`, raw archives, backups, and rollback
targets. Keep coverage that loads or validates each target family through the
owning domain contract where practical; archive, backup, and rollback files are
Migration-owned and need explicit migration fixture assertions.

## Safety Invariants

Tests that touch user files must prove the owning module preserves these rules:

- Writes stay inside managed repo paths unless the command explicitly owns an
  external install destination.
- Symlinked managed artifacts are rejected where the owning contract requires
  path containment.
- Managed blocks and install-owned files do not delete user content.
- Failed writes roll back already-written files when the operation promises
  rollback.
- Interrupted writes have an explicit recovery or reconciliation path before
  later commands trust ownership records.
- Backups do not overwrite existing backup destinations.
- Schema mismatch errors name the expected and found schema versions.
- Optimistic concurrency failures do not silently overwrite newer task data.
- Cross-domain outcome application checks the source snapshot before mutating
  the target artifact.
- Concurrent appends to managed JSONL artifacts preserve complete lines or leave
  readers able to ignore incomplete trailing data.
- Dry-run and check modes do not write state.

## Test Style

Prefer contract tests at the owning domain module boundary. Adapter tests should
assert parsing, routing, rendering, and exit behavior without re-testing every
domain rule.

Command integration tests should run the compiled `maestro` binary through
`CARGO_BIN_EXE_maestro` and use isolated temporary repositories.

Safety tests should assert both the final artifact state and the absence of
partial or unintended files after failure.

Read-model tests should build source artifacts directly when the purpose is the
projection, and use runtime flows when the purpose is cross-module composition.

When adding a new module, command, artifact schema, or runtime flow, update:

- `ARCHITECTURE.md` module ownership and Testing Map.
- This file's Module Test Matrix.
- At least one contract test for the owning module.
- Adapter or runtime-flow tests for user-facing behavior.
