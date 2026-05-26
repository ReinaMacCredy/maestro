# Maestro Maintenance Guide

This guide defines how to keep the Rust Maestro codebase easy to change over
time. Use `ARCHITECTURE.md` for ownership and target direction. Use
`TESTING.md` for verification selection. Use this file for maintenance habits,
refactor discipline, docs drift control, and handoff expectations.

## Maintenance Goals

Maestro should stay local-first, predictable, and easy for future agents to
navigate. A maintainer should be able to answer these questions before editing:

- Which domain contract am I touching?
- Which artifacts can this module read or mutate?
- Which tests prove this change?
- Which docs must change if this contract moves?
- What user data or local agent state could be affected?

If those answers are unclear, improve the contract or docs before widening the
implementation.

## Routine Change Loop

Before editing:

- Read the closest relevant ownership section in `ARCHITECTURE.md`.
- Read the relevant test row in `TESTING.md`.
- Inspect the current code path with `rg` and narrow file reads.
- Identify whether the change is private implementation, public contract,
  adapter behavior, artifact schema, or user-data safety behavior.

During editing:

- Keep changes scoped to the owning module and its direct callers.
- Prefer moving duplicated rules into the owning domain module instead of
  adding another adapter-side copy.
- Preserve current behavior unless the task explicitly asks for a behavior
  change.
- Add or update the smallest contract test that proves the changed invariant.
- When moving a cross-domain behavior behind an operation, preserve the
  operation's transaction, concurrency, and recovery semantics before moving the
  next caller.

Before handoff:

- Run the smallest targeted checks that can falsify the touched contract.
- Broaden checks when a public contract, schema, path layout, or safety policy
  changed.
- Read back any docs you changed.
- Check the diff for unrelated edits, stale wording, and accidental scope creep.

For code changes, `TESTING.md` is the canonical verification source. The
default final gate is:

```sh
cargo check --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For docs-only changes, use:

```sh
git diff --check -- ARCHITECTURE.md TESTING.md MAINTENANCE.md
```

Then read back the changed sections. If the change touches repo-local agent
instructions, include the tracked agent file path for this checkout, such as
`AGENTS.md` or `AGENTS.MD`.

## Refactor Discipline

Use the Known Refactor Seams in `ARCHITECTURE.md` as the preferred order of
attack. Start with high-leverage boundaries before polishing lower-impact
adapters.

Do refactors in slices:

1. Capture the existing behavior with a focused test if it is not already
   protected.
2. Introduce or tighten the owning module interface.
3. Move one caller to the interface.
4. Run the owning module tests and the caller's adapter or flow tests.
5. Repeat for the next caller.
6. Remove the old duplicated rule only after all callers have moved.

Avoid big-bang rewrites across Task, Proof, Run, Install, Migration, and
adapters at the same time. Those modules touch durable local artifacts, so
smaller compatibility-preserving slices are easier to verify and recover from.

Do not add a new abstraction only because a boundary might exist later. Add an
interface when it protects a real contract, removes real duplication, enables a
needed test seam, or matches an established module pattern.

## Internal Module Layout

Use the Option 2 layout from `ARCHITECTURE.md`: a module facade plus private
internal folders only where the implementation is large enough to need them.

During the migration from the flat crate-root layout to `interfaces/`,
`domain/`, `operations/`, and `foundation/`, compatibility aliases are temporary
shims. They preserve old import paths around moved behavior, but they must not
grow duplicate behavior or speculative public APIs. When a module actually
moves, update the facade and `tests/architecture_imports.rs` in the same slice
before removing the old path.

Current migration state: Core implementation files live under
`src/foundation/core`, and `crate::core` remains a compatibility re-export.
New production imports should prefer `crate::foundation::core`; keep
`crate::core` and `maestro::core` usage to compatibility checks or transition
work until the legacy path is removed.

CLI adapter files live under `src/interfaces/cli`, and `crate::commands`
remains a compatibility re-export. New production references should prefer
`crate::interfaces::cli`; current CLI imports into legacy domain, operation,
and interface roots are temporary allowances until those facades move in later
phases.

Shell integration lives under `src/interfaces/shell`, and `crate::shell` remains
a compatibility re-export. New production references should prefer
`crate::interfaces::shell`; keep `crate::shell` and `maestro::shell` usage to
compatibility checks or transition work until the legacy path is removed.

MCP integration lives under `src/interfaces/mcp`, and `crate::mcp` remains a
compatibility re-export. New production references should prefer
`crate::interfaces::mcp`; keep `crate::mcp` and `maestro::mcp` usage to
compatibility checks or transition work until the legacy path is removed.
Current MCP tool imports into legacy source-read roots are temporary,
file-specific allowances until those domain read facades move in later phases.

Hook command adapter code lives under `src/interfaces/hooks`, and `crate::hooks`
remains a compatibility re-export. New interface-layer references should prefer
`crate::interfaces::hooks`. Non-interface callers that still need run-event
helpers may keep using `crate::hooks` until the later Run aggregate extraction
moves those helpers behind a non-interface facade.

When adding a folder inside a domain or operations module:

- Keep the parent `mod.rs` as the caller-facing facade.
- Name the folder after a real sub-concept, such as `repository`, `lifecycle`,
  `acceptance`, `runner`, `apply`, or `safety`.
- Keep deep leaf files private unless the architecture spec explicitly exposes a
  child facade.
- Use private `mod` declarations and explicit `pub use` re-exports for the
  public contract. Avoid `pub mod` for internal folders unless the child module
  is intentionally part of the public facade.
- Move tests toward the module contract rather than the leaf file shape.
- Preserve edge-case coverage when moving tests. Do not delete a leaf-level test
  until the same behavior is covered through the facade, or until the behavior
  is explicitly marked implementation-only and no longer relevant.
- Keep parent `mod.rs` small: docs, module declarations, public re-exports, and
  contract-level constructors or queries only. Move implementation logic into
  named private files.

Do not introduce full `ports/`, `adapters/`, `use_cases/`, or `services/`
stacks inside every module. Use that heavier shape only after a module has
multiple real adapters, multiple independent flows, and a stable domain model
that the simple facade shape no longer protects.

## Docs Drift Rules

Update docs in the same change when behavior changes the contract.

| Change | Required docs check |
| --- | --- |
| New module, command, artifact, or runtime flow | Update `ARCHITECTURE.md` and `TESTING.md`. |
| Changed module ownership or dependency direction | Update `ARCHITECTURE.md`. |
| Changed verification command, test file, or test responsibility | Update `TESTING.md`. |
| Changed maintenance workflow, release-adjacent routine, or handoff rule | Update `MAINTENANCE.md`. |
| Changed agent operating rule that must apply in every future session | Update repo-local `AGENTS.md`. |
| Changed generated Harness content or installed agent-facing file | Update the owning doc plus tests that assert generated text. |
| Moved embedded prose/templates into resource files | Update `ARCHITECTURE.md` resource policy and `TESTING.md` resource coverage. |

Do not copy long sections between docs. Link the responsibility instead:

- `ARCHITECTURE.md` owns the map.
- `TESTING.md` owns verification selection.
- `MAINTENANCE.md` owns operating discipline.
- `AGENTS.md` owns always-on agent instructions.

Human-authored templates and agent-facing prose should be edited as resource
files, not as long Rust raw strings. Rust code should embed those resources at
compile time and own validation, serialization, rollback, and install policy.

Bundled skills are directory packages. Preserve `SKILL.md`, `references/`,
`scripts/`, and `assets/` as authored resource files. Treat `evals/` as
development material unless a runtime skill explicitly needs it. When scripts
ship with a skill, preserve executable metadata through extraction, install,
update, and rollback.

## Safety-Sensitive Surfaces

Treat these surfaces as higher risk because they affect user files, local agent
state, or durable migration output:

| Surface | Maintenance rule |
| --- | --- |
| `install` and `skills` | Preserve install-lock ownership, pending/committed recovery state, rollback, managed-block boundaries, and skill symlink safety. |
| `migrate` | Keep dry-run/check behavior non-mutating, preserve backups, document direct-write exceptions, and verify migrated artifacts through target-domain loaders where possible. |
| `update` | Keep check/update behavior separate, preserve rollback for downloaded or extracted files, report schema drift clearly, and do not apply Harness changes silently. |
| `task` | Preserve optimistic concurrency, acceptance locking, state history, blockers, and verification binding semantics. |
| `verification` | Keep Proof-owned reports separate from Task-owned lifecycle effects and preserve stale-snapshot handling for Proof-to-Task outcome application. |
| `hooks` and `evidence` | Preserve event normalization, session encoding, append concurrency, symlink rejection, partial-line tolerance, and run evidence derivation. |
| `harness backlog` | Preserve backlog schema, duplicate handling, deterministic ordering, refresh-without-apply, and read-model behavior. |
| `core` | Keep helpers domain-neutral. Do not move Task, Proof, Install, or Migration policy into Core. |

When changing any safety-sensitive surface, run the module tests plus at least
one runtime-flow test that exercises the user-visible path.

## Artifact Compatibility

Durable artifacts need compatibility discipline:

- Keep schema versions explicit and tested.
- Reject unsupported schema versions with errors that name expected and found
  versions.
- Do not silently rewrite user-owned Harness files, install mirrors, decisions,
  or task artifacts outside explicit commands that own the mutation.
- Treat reserved schema constants as inactive until the corresponding artifact
  format, migration behavior, and tests are documented.
- Add Migration support when an old supported artifact shape must keep working.
- Prefer additive schema changes when they can preserve old readers.
- If a breaking artifact change is unavoidable, document the migration path and
  prove rollback or recovery behavior.

## Adapter Maintenance

Adapters include CLI commands, MCP tools, hook command handling, shell output,
and TUI surfaces. They should translate external input into domain calls and
render domain output.

Adapter code should not:

- Allocate domain ids when the domain module can own allocation.
- Mutate durable domain artifacts directly when a domain operation exists.
- Coordinate multi-domain workflows directly when an `operations/` module owns
  the sequence.
- Duplicate lifecycle, freshness, migration, or install ownership rules.
- Depend on private artifact layout when a read model exists.

When an adapter grows domain behavior, treat that as a refactor seam. Move the
rule behind the owning module before adding more callers.

## Release-Adjacent Checks

This Rust codebase is Cargo-first. Before any release-adjacent handoff, run the
full Rust verification gate from `TESTING.md` and inspect current release or
update-specific instructions in the repo before assuming a packaging flow.

At minimum for release-adjacent changes:

- Run `cargo check --all-targets`.
- Run `cargo fmt --check`.
- Run `cargo clippy --all-targets -- -D warnings`.
- Run `cargo test`.
- Run `cargo doc --no-deps` when public docs or public Rust APIs changed.
- Run Update, Install, Skills, and Migration tests when release behavior affects
  installed files, bundled skills, update detection, or artifact migration.

Do not document or run a legacy release command unless it exists in the current
Rust repo and has been verified in the current session.

## Agent Handoff Standard

Every non-trivial maintenance handoff should state:

- The touched contract or module.
- The files changed.
- The behavior preserved or intentionally changed.
- The tests or checks run.
- Any checks skipped and why.
- Any remaining risk or follow-up.

If a change intentionally leaves a known refactor seam unresolved, name the seam
from `ARCHITECTURE.md` instead of hiding it in vague future-work language.

If the work depends on current machine state, installed binaries, live MCP
servers, or local agent config, verify the live state before claiming it works.

## Keeping This Guide Current

Update this file when maintenance behavior changes. Examples:

- The verification gate changes.
- A release, update, or install routine becomes concrete.
- A new safety-sensitive module is added.
- A recurring handoff problem needs an always-documented rule.
- The architecture doc gains a new refactor seam that changes maintenance
  priorities.

Keep this guide operational. If a rule does not change what a maintainer should
do, remove or sharpen it.
