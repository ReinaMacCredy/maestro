# Provisional CLI Harness Interface

Feature: `optimize-test-runtime-and-architecture`
SPEC: ./SPEC-test-optimization.md
Inventory: ./TEST-INVENTORY-test-optimization.md
VERIFY: ./VERIFY-test-optimization.md

## Status

Provisional design for T2. This is not an implementation and does not freeze API names. It defines the shape T3 should implement minimally, then validate through representative migrations.

## Design target

Create a generic process harness for the `maestro` binary. The harness owns process mechanics only:

- command construction for `env!("CARGO_BIN_EXE_maestro")`
- explicit working directory
- explicit environment additions/removals
- optional stdin
- optional timeout/spawn handling
- raw status/stdout/stderr capture
- explicit success/failure assertions and diagnostics

Domain fixtures stay outside the harness:

- `TestTempDir` remains a fixture/lifecycle utility, not a process concern.
- `cards_repo`, `init_repo`, `setup_repo`, and domain seeders remain in fixture modules.
- card/session/domain-specific defaults must not be baked into the generic harness.
- `Command::new("git")` helpers stay out of the `maestro` harness.

## Proposed module shape

Preferred initial path for T3:

```text
tests/common/mod.rs
tests/common/cli_harness.rs
```

Each integration test imports it with:

```rust
mod common;
use common::cli_harness::{maestro, MaestroOutput};
```

This keeps the harness test-local and avoids exposing a production crate interface.

## Provisional interface

API names are provisional. The important interface shape is small and explicit:

```rust
pub fn maestro(cwd: &Path) -> MaestroCmd;

pub struct MaestroCmd { /* private */ }

impl MaestroCmd {
    pub fn args(mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self;
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self;
    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Self;
    pub fn stdin(mut self, text: impl Into<Vec<u8>>) -> Self;
    pub fn timeout(mut self, duration: Duration) -> Self;
    pub fn output(self) -> MaestroOutput;
    pub fn spawn(self) -> MaestroChild;
}

pub struct MaestroOutput { /* wraps std::process::Output */ }

impl MaestroOutput {
    pub fn status(&self) -> ExitStatus;
    pub fn stdout(&self) -> String;
    pub fn stderr(&self) -> String;
    pub fn assert_success(&self, context: impl Display);
    pub fn assert_failure(&self, context: impl Display);
    pub fn into_raw(self) -> Output;
}
```

The harness should prefer owned/borrow-flexible argument inputs only if it does not add noisy generic complexity. A simpler `args(&[&str])` is acceptable for the first slice if it covers representative migrations cleanly.

## Conservative defaults

Default behavior must preserve current process semantics:

- No automatic success assertion. Tests opt into `assert_success` or inspect raw `status`.
- No default `MAESTRO_AGENT`, `MAESTRO_SESSION`, `MAESTRO_SESSION_ID`, `MAESTRO_AUTO_UPDATE`, or `HOME` values.
- No automatic stderr normalization or stdout trimming.
- No automatic timeout. Timeout is opt-in for the small subset of spawn/timeout tests.
- No implicit temp repo creation or fixture seeding.
- No global current-directory mutation; every command has an explicit `cwd`.

## Migration buckets from inventory

### Straight-through `Output` helpers

Current shape examples: `fn maestro(cwd, args) -> Output`, `fn maestro(args, cwd) -> Output`.

Mechanical target:

```rust
let output = maestro(repo).args(args).output();
```

Then keep existing assertions unchanged or move only local `stdout` / `stderr` decoding when safe.

### Env/session variants

Current shape examples: `maestro_with_env`, `maestro_in_session`, helpers setting `HOME`, `MAESTRO_AGENT`, `MAESTRO_SESSION`, `MAESTRO_SESSION_ID`, `MAESTRO_RUN_ID`, `MAESTRO_CURRENT_TASK`.

Mechanical target:

```rust
let output = maestro(repo)
    .args(args)
    .env("MAESTRO_AGENT", "codex")
    .env("MAESTRO_SESSION_ID", session)
    .output();
```

Do not add convenience methods such as `.codex_session()` until at least two migrated tests show the same non-domain-specific need.

### Stdin helpers

Current shape examples: `maestro_with_stdin`, `run_evidence`, QA-gate helpers.

Mechanical target:

```rust
let output = maestro(repo)
    .args(args)
    .stdin(observed)
    .output();
```

The harness writes stdin bytes and closes the pipe; tests still assert stdout/stderr/status explicitly.

### Timeout/spawn helpers

Current shape examples: `maestro_with_timeout`, MCP process helpers, hook/session tests that use `spawn`, `try_wait`, `thread::sleep`, or `Duration::from_secs`.

Mechanical target:

```rust
let output = maestro(repo)
    .args(args)
    .timeout(Duration::from_secs(10))
    .output();
```

`spawn()` support should remain minimal and explicit. Do not convert all timeout tests in the first slice; migrate one representative case only after straight-through output helpers pass.

### Assertion helpers

Current shape examples: `assert_success`, `assert_failure`, `stdout`, `stderr` duplicated in many files.

Mechanical target:

```rust
let output = maestro(repo).args(args).output();
output.assert_success(format_args!("maestro {args:?}"));
let out = output.stdout();
```

Assertions should print command context plus raw stdout/stderr. They must not hide raw `Output` access.

## First T3 migration candidates

Pick low-risk files that cover distinct patterns without global-state or timeout complexity first:

1. `tests/did_you_mean_integration.rs` -- simple CLI output helper and stdout decode.
2. `tests/unknown_subcommand_integration.rs` -- simple command/error behavior without fixture-heavy setup.
3. `tests/design_integration.rs` -- explicit `HOME` env and cwd handling.
4. `tests/feature_qa_gate_integration.rs` -- representative stdin helper, only after the straight-through path is stable.

Avoid first-slice migrations in:

- `tests/feature_decision_commands_integration.rs` because it has timeout/spawn/race coverage.
- `tests/harness_integration.rs` because it mixes process, MCP, timeout, and many fixture helpers.
- `tests/card_commands_integration.rs` because it is large and card/session-heavy.

## Verification expectations for T3

A T3 implementation proves the interface only if:

- representative migrated tests preserve existing assertions and failure messages closely enough for maintainers to diagnose failures;
- domain fixtures remain in existing fixture modules;
- raw `Output` remains accessible;
- env/cwd/stdin/timeout behavior is opt-in and explicit;
- no timing thresholds, CI split, skip policy, or broad migration order is introduced.
