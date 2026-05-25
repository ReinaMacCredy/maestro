# Plan: Maestro Rust Spec v0.8 Implementation

**Generated**: 2026-05-25
**Source spec**: `SPEC-v0.8.md`
**Goal-loop invocation**: `Use $goal-loop PLAN-v0.8.md`
**Implementation target**: greenfield Rust `maestro` CLI in this repo

## Goal

Implement `SPEC-v0.8.md` as a local-first Rust CLI. The finished V1 creates and maintains `.maestro/` as the repo-local substrate for Claude Code and Codex, with tasks, features, decisions, bundled skills, hooks, verification, computed metrics, harness improver, `task list --watch`, MCP server, and migration from v0.106.1.

## Non-Goals

- Do not port dropped v0.7 systems: missions, verdict/trust verifier, AI reviewer, auto-merge, multi-pane TUI, worktrees, bundles, policies, separate intake, separate handoffs, SQLite metrics, or cache/tmp dirs.
- Do not add LLM calls, telemetry, background daemons, cloud sync, or agent launching.
- Do not implement Windows-native support or npm wrapper in V1.

## Global Constraints

- Follow `AGENTS.MD` Rust guidance: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`; avoid `unwrap()` outside tests unless justified by `expect("invariant: ...")`.
- Keep artifact source of truth in files under `.maestro/`; derived views compute on demand.
- Preserve config-safe mirror writes with managed blocks/keys and install lock ownership.
- Treat hooks as observe-only and always exit 0 in V1.
- Keep all phase commits scoped. Never stage `IMPLEMENTATION_NOTES.md`.

## Phase Status

- Phase 1 - Foundation: done
- Phase 2 - Artifact protocol: todo
- Phase 3 - Core verbs: todo
- Phase 4 - Hooks and skills: todo
- Phase 5 - MCP, metrics, improver: todo
- Phase 6 - TUI and migration: todo

## Subsystem Coverage Matrix

| System | Phase |
|---|---|
| Init + HARNESS.md generation | 2 |
| install/update/uninstall config-safe mirrors | 2, 4 |
| Hook adapter `maestro hook record` | 4 |
| Task lifecycle, blockers, state_history | 2, 3 |
| Bundled skills and symlink mirrors | 4 |
| Config-safe install lock, backups, diffs | 1, 2 |
| Verification and proof freshness | 3 |
| Decision records | 2, 3 |
| Feature subsystem | 2, 3 |
| Computed metrics | 5 |
| Harness improver | 5 |
| `task list --watch` | 6 |
| Migration from v0.106.1 | 6 |
| MCP server | 5 |

## Phase 1 - Foundation

**Goal**: Create a compiling Rust CLI skeleton and reusable filesystem, schema, git, diff, backup, and managed-block primitives.

**Demo/Validation**:

- `cargo build`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `maestro --help` shows root command shell

### Task 1.1 - Bootstrap Cargo CLI

- **Location**: `Cargo.toml`, `src/main.rs`, `src/commands/mod.rs`
- **Description**: Create the Rust package, `clap` root CLI, command module layout from spec section 41, and placeholder command dispatch that compiles.
- **Dependencies**: none
- **Writes**: `Cargo.toml`, `src/main.rs`, `src/commands/mod.rs`
- **Acceptance Criteria**:
  - Binary name is `maestro`.
  - CLI has top-level help and placeholder subcommands matching section 38.
  - Dependencies include only V1-approved crates from section 41.
- **Validation**: `cargo build`, `cargo test`

### Task 1.2 - Core path and filesystem primitives

- **Location**: `src/core/paths.rs`, `src/core/fs.rs`, `src/core/safe_write.rs`, `src/core/mod.rs`
- **Description**: Add repo-root discovery, `.maestro/` path helpers, atomic/safe write helpers, parent directory creation, and test utilities.
- **Dependencies**: Task 1.1
- **Writes**: `src/core/paths.rs`, `src/core/fs.rs`, `src/core/safe_write.rs`, `src/core/mod.rs`
- **Acceptance Criteria**:
  - Helpers resolve `.maestro/` from an explicit or current working directory.
  - Writes are atomic enough for local CLI use.
  - Errors are recoverable `Result`s with context.
- **Validation**: focused unit tests

### Task 1.3 - Schema constants and typed errors

- **Location**: `src/core/schema.rs`, `src/core/error.rs`
- **Description**: Define V1 schema version constants and typed error variants used by artifact modules.
- **Dependencies**: Task 1.1
- **Writes**: `src/core/schema.rs`, `src/core/error.rs`
- **Acceptance Criteria**:
  - All 11 schema versions from section 37 are represented.
  - Public types implement `Debug`.
- **Validation**: unit tests for constants and error display

### Task 1.4 - Managed block and JSON key ownership primitives

- **Location**: `src/core/managed_blocks.rs`
- **Description**: Implement managed block insertion/removal for Markdown, TOML/gitignore-style comments, and managed JSON key merging/removal.
- **Dependencies**: Tasks 1.2, 1.3
- **Writes**: `src/core/managed_blocks.rs`
- **Acceptance Criteria**:
  - User content outside markers is preserved.
  - Missing files can be created as fully managed.
  - Deleted managed blocks can be reinstalled.
  - JSON managed keys are namespaced and reversible.
- **Validation**: unit tests covering Markdown, gitignore, TOML, JSON, user edits, and removal

### Task 1.5 - Backup, diff preview, and git helpers

- **Location**: `src/core/backup.rs`, `src/core/diff.rs`, `src/core/git.rs`
- **Description**: Implement timestamped backups, unified diff previews, git HEAD/status helpers, and a narrow wrapper around `git2`.
- **Dependencies**: Tasks 1.2, 1.4
- **Writes**: `src/core/backup.rs`, `src/core/diff.rs`, `src/core/git.rs`
- **Acceptance Criteria**:
  - Backups write under `.maestro/backups/<timestamp>-<operation>/`.
  - Diff previews show before/after for install/migration/update.
  - Git helpers support proof freshness and migration checks.
- **Validation**: unit tests with temp repos

## Phase 2 - Artifact Protocol

**Goal**: Implement the canonical `.maestro/` artifact shape, `init`, `install --agent`, task lifecycle storage, feature storage, and decision templates.

**Demo/Validation**:

- `maestro init --dry-run`
- `maestro init --yes`
- `maestro install --agent codex --dry-run` if dry-run exists for mirrors, otherwise inspect diff preview
- Artifact tree matches spec section 5

### Task 2.1 - Harness config and init templates

- **Location**: `src/commands/init.rs`, `src/harness/`, `src/core/paths.rs`
- **Description**: Implement `maestro init` flags and generate `.maestro/harness/{HARNESS.md,harness.yml,backlog.yaml}`, `.maestro/features/features.yaml`, `.maestro/decisions/`, and `.maestro/skills/`.
- **Dependencies**: Phase 1
- **Writes**: `src/commands/init.rs`, `src/harness/mod.rs`, `src/harness/templates.rs`, `src/harness/schema.rs`
- **Acceptance Criteria**:
  - `--dry-run`, `--merge`, `--force`, and `--yes` match Appendix A.11.
  - Unknown stack fallback follows Appendix A.4.
  - Generated `HARNESS.md` matches section 14.
- **Validation**: integration tests in temp repos

### Task 2.2 - Install lock and mirror writes

- **Location**: `src/commands/install.rs`, `src/commands/uninstall.rs`, `src/core/managed_blocks.rs`
- **Description**: Implement install lock, backups, `.gitignore` managed block, `CLAUDE.md`, `AGENTS.md`, `.claude/settings.local.json`, `.codex/hooks.json`, `.codex/config.toml`, and mirror uninstall.
- **Dependencies**: Tasks 1.4, 1.5, 2.1
- **Writes**: `src/commands/install.rs`, `src/commands/uninstall.rs`, `src/install/mod.rs`, `src/install/lock.rs`, `src/install/mirrors.rs`
- **Acceptance Criteria**:
  - `install --agent claude|codex` writes only managed content and records ownership in `.maestro/install-lock.yaml`.
  - Codex install prints manual `/hooks` trust reminder.
  - Uninstall removes only managed blocks/keys it owns.
  - Catastrophic exceptions from Appendix A.12 are enforced.
- **Validation**: integration tests for fresh files, existing user content, edited managed blocks, deleted blocks, uninstall

### Task 2.3 - Feature and decision artifact modules

- **Location**: `src/feature/schema.rs`, `src/feature/query.rs`, `src/decisions/template.rs`
- **Description**: Implement feature registry load/save/query and decision markdown template creation.
- **Dependencies**: Tasks 1.2, 1.3
- **Writes**: `src/feature/schema.rs`, `src/feature/query.rs`, `src/feature/mod.rs`, `src/decisions/template.rs`, `src/decisions/mod.rs`
- **Acceptance Criteria**:
  - `features.yaml` schema matches section 7.1.
  - Feature task counts are computed from tasks, not stored in feature records.
  - Decision files use `decision-NNN-<slug>.md` naming and section 7.4 template.
- **Validation**: unit tests with temp `.maestro/`

### Task 2.4 - Task artifact model and lifecycle core

- **Location**: `src/task/lifecycle.rs`, `src/task/blockers.rs`, `src/task/template.rs`
- **Description**: Implement task directory creation, `task.yaml`, `task.md`, `acceptance.yaml`, lifecycle transitions, blockers, state_history, optimistic concurrency, and proof-state helpers.
- **Dependencies**: Tasks 1.2, 1.3, 2.3
- **Writes**: `src/task/lifecycle.rs`, `src/task/blockers.rs`, `src/task/template.rs`, `src/task/mod.rs`
- **Acceptance Criteria**:
  - Task schema matches section 7.2 and Appendix A.8.
  - Transition gates match section 10.3.
  - Blockers are overlay metadata, not states.
  - Optimistic concurrency checks `updated_at`.
- **Validation**: unit tests for every transition, blocker add/remove, stale updated_at conflict, proof states

## Phase 3 - Core Verbs

**Goal**: Expose the task, feature, decision, verification, shell-init, query, and doctor CLI surfaces.

**Demo/Validation**:

- Create/explore/accept/claim/complete/verify a task in a temp repo.
- `maestro task show`, `task list`, `task doctor`, `feature list`, `decision list`, `query proof` all work.

### Task 3.1 - Task command surface

- **Location**: `src/commands/task.rs`
- **Description**: Wire all 11 task verbs plus reads to task lifecycle, blockers, display, and filters.
- **Dependencies**: Task 2.4
- **Writes**: `src/commands/task.rs`, `src/task/display.rs`
- **Acceptance Criteria**:
  - CLI matches section 10.2 and section 38.
  - `task claim` sets `claimed_by` and progresses ready to in_progress.
  - `task show` with no id reads `MAESTRO_CURRENT_TASK`.
  - Terminal verbs append irreversible state_history entries.
- **Validation**: CLI integration tests

### Task 3.2 - Feature and decision command surface

- **Location**: `src/commands/feature.rs`, `src/commands/decision.rs`
- **Description**: Implement feature new/show/list/edit/ship/cancel and decision new/show/list.
- **Dependencies**: Task 2.3
- **Writes**: `src/commands/feature.rs`, `src/commands/decision.rs`
- **Acceptance Criteria**:
  - Feature views compute task counts by scanning task.yaml.
  - Decision ids auto-increment and preserve markdown template.
- **Validation**: CLI integration tests

### Task 3.3 - Verification command and proof freshness

- **Location**: `src/commands/verify.rs`, `src/verification/verify_task.rs`, `src/verification/proof_status.rs`, `src/verification/stale.rs`
- **Description**: Implement `task verify`, `query proof`, command execution from `harness.yml.verify`, proof hash binding, and claim cross-checking against events.
- **Dependencies**: Tasks 2.4, 3.1
- **Writes**: `src/commands/verify.rs`, `src/verification/verify_task.rs`, `src/verification/proof_status.rs`, `src/verification/stale.rs`, `src/verification/mod.rs`
- **Acceptance Criteria**:
  - `task verify` owns `needs_verification -> verified`.
  - `verification.json` matches section 24.
  - Claims are checked against `events.jsonl` per Appendix A.2.
  - Proof states missing/failed/accepted/stale compute correctly.
- **Validation**: tests for pass, fail, stale HEAD, stale acceptance, stale checks, unsupported claims

### Task 3.4 - Shell integration

- **Location**: `src/commands/shell_init.rs`
- **Description**: Implement `maestro shell-init` for zsh/bash/fish and side-channel support for current task updates.
- **Dependencies**: Task 3.1
- **Writes**: `src/commands/shell_init.rs`, `src/shell/mod.rs`
- **Acceptance Criteria**:
  - `task claim` can set `MAESTRO_CURRENT_TASK` through the shell wrapper.
  - `task complete` can unset it after success.
  - Each terminal remains independent.
- **Validation**: snapshot tests for shell snippets and command integration tests

### Task 3.5 - Doctor and read-only query commands

- **Location**: `src/commands/doctor.rs`, `src/commands/query.rs`, `src/task/doctor.rs`
- **Description**: Implement `maestro doctor`, `task doctor`, and computed queries: matrix, friction, decisions, backlog, proof.
- **Dependencies**: Tasks 2.4, 3.2, 3.3
- **Writes**: `src/commands/doctor.rs`, `src/commands/query.rs`, `src/task/doctor.rs`
- **Acceptance Criteria**:
  - Blocker graph checks match section 10.6.
  - Query commands walk files on demand and do not write caches.
- **Validation**: integration tests for all doctor severities and query outputs

## Phase 4 - Hooks And Skills

**Goal**: Implement passive hook ingestion, run evidence generation, hook config writers, bundled skill extraction, skill symlink mirrors, update, and skill activation logging.

**Demo/Validation**:

- Pipe sample Claude/Codex hook JSON into `maestro hook record`.
- `maestro install --agent claude|codex` writes hook configs and symlink mirrors.
- `maestro update` re-extracts bundled skills with backup behavior.

### Task 4.1 - Hook event normalization and append path

- **Location**: `src/commands/hook.rs`, `src/hooks/record.rs`
- **Description**: Implement `maestro hook record` stdin parsing, event normalization to `maestro.event.v1`, privacy hashing, run attribution, append-only JSONL, and always-exit-0 failure mode.
- **Dependencies**: Tasks 1.2, 1.3, 2.2
- **Writes**: `src/commands/hook.rs`, `src/hooks/record.rs`, `src/hooks/mod.rs`
- **Acceptance Criteria**:
  - Six shared events are accepted.
  - Missing session id writes to `runs/unattributed/events.jsonl`.
  - `tool_input_hash` stores hashes, not raw content.
  - Adapter exits 0 even on write failure.
- **Validation**: unit tests with sample payloads, malformed payloads, missing session id, simulated write failure

### Task 4.2 - Run evidence generation

- **Location**: `src/hooks/record.rs`, `src/evidence/`
- **Description**: On Stop events, aggregate events into `run_evidence.yaml` and close run metadata.
- **Dependencies**: Task 4.1
- **Writes**: `src/evidence/mod.rs`, `src/evidence/run_evidence.rs`, `src/hooks/record.rs`
- **Acceptance Criteria**:
  - `run_evidence.yaml` matches section 18.
  - Tool counts, human interventions, duration, start/end commits are computed from events.
- **Validation**: fixture tests over event streams

### Task 4.3 - Hook config writers

- **Location**: `src/install/hooks.rs`, `src/commands/install.rs`
- **Description**: Extend install/update/uninstall to write Claude and Codex hook config shapes from section 16.3.
- **Dependencies**: Tasks 2.2, 4.1
- **Writes**: `src/install/hooks.rs`, `src/commands/install.rs`, `src/commands/uninstall.rs`
- **Acceptance Criteria**:
  - Claude writes managed `.claude/settings.local.json` keys.
  - Codex writes managed `.codex/hooks.json` entries with timeout 5.
  - Uninstall removes managed hook config only.
- **Validation**: integration tests for install/uninstall and user key preservation

### Task 4.4 - Bundled skills extraction and symlink mirrors

- **Location**: `src/skills/bundled.rs`, `src/skills/extract.rs`, `src/skills/symlink.rs`
- **Description**: Embed and extract four bundled skills, walk `.maestro/skills/` directly, create `.claude/skills` and `.codex/skills` symlinks during install, and log skill activations.
- **Dependencies**: Tasks 2.1, 2.2, 4.1
- **Writes**: `src/skills/bundled.rs`, `src/skills/extract.rs`, `src/skills/symlink.rs`, `src/skills/mod.rs`
- **Acceptance Criteria**:
  - Bundled list is exactly `maestro-task`, `maestro-setup`, `maestro-verify`, `maestro-design`.
  - No `skill-index.yaml` is created.
  - User-added skills remain untouched.
  - Skill activation events append to events.jsonl.
- **Validation**: extraction tests, symlink tests, activation event test

### Task 4.5 - Update command

- **Location**: `src/commands/update.rs`, `src/skills/extract.rs`
- **Description**: Implement `maestro update` per Appendix A.1, including binary download placeholder/seam, checksum seam, atomic replace seam, bundled skill re-extraction, backups, and migrate prompt.
- **Dependencies**: Tasks 1.5, 4.4
- **Writes**: `src/commands/update.rs`, `src/update/mod.rs`
- **Acceptance Criteria**:
  - Existing binary remains usable on failure.
  - User-edited bundled skills are backed up before overwrite.
  - Schema mismatch prompts `maestro migrate`.
- **Validation**: tests for skill overwrite paths and failure preservation; network download can be abstracted behind a trait in V1 tests

## Phase 5 - MCP, Metrics, Improver

**Goal**: Implement active MCP server, computed metrics, friction queries, and rule-based harness improver.

**Demo/Validation**:

- `maestro mcp serve` starts a stdio server exposing the listed tools.
- `maestro metrics summary` and `maestro improve list/show/apply` operate from file artifacts only.

### Task 5.1 - MCP server and tools

- **Location**: `src/commands/mcp.rs`, `src/mcp/server.rs`, `src/mcp/tools.rs`
- **Description**: Implement stdio MCP server with the 13 tools from section 31 and Appendix A.9.
- **Dependencies**: Phases 2-4
- **Writes**: `src/commands/mcp.rs`, `src/mcp/server.rs`, `src/mcp/tools.rs`, `src/mcp/mod.rs`
- **Acceptance Criteria**:
  - Tools expose read and mutation surfaces exactly as listed.
  - No network port or daemon is created.
  - Mutations reuse CLI logic, not parallel implementations.
- **Validation**: MCP protocol smoke tests and direct tool handler tests

### Task 5.2 - Computed metrics summary

- **Location**: `src/commands/metrics.rs`, `src/metrics/summary.rs`
- **Description**: Implement computed metrics from tasks and runs with no SQLite or cache.
- **Dependencies**: Tasks 2.4, 4.2
- **Writes**: `src/commands/metrics.rs`, `src/metrics/summary.rs`, `src/metrics/mod.rs`
- **Acceptance Criteria**:
  - Output includes task counts, average time-to-verify, per-agent counts, and interventions per task.
  - Reads file artifacts on demand.
- **Validation**: fixture tests with tasks and run_evidence files

### Task 5.3 - Friction query and harness improver detection

- **Location**: `src/improver/detect.rs`, `src/improver/propose.rs`, `src/commands/improve.rs`, `src/metrics/friction.rs`
- **Description**: Implement rule-based detection from Appendix A.7 and backlog proposal write/apply flow.
- **Dependencies**: Tasks 2.1, 4.2, 5.2
- **Writes**: `src/improver/detect.rs`, `src/improver/propose.rs`, `src/improver/mod.rs`, `src/commands/improve.rs`, `src/metrics/friction.rs`
- **Acceptance Criteria**:
  - No LLM calls.
  - Proposals write to `.maestro/harness/backlog.yaml`.
  - User applies via `maestro improve apply <id>`.
- **Validation**: fixture tests for all five detection rules

## Phase 6 - TUI And Migration

**Goal**: Implement `task list --watch`, v0.106.1 migration check/apply, and final V1 demo gate.

**Demo/Validation**:

- `maestro task list --watch --interval 1` renders the sandcastle-style layout.
- `maestro migrate --check` produces a diff without writing.
- `maestro migrate` applies with backups and refuses concurrent writers unless forced.
- Full V1 demo from section 42 runs in a temp repo.

### Task 6.1 - Task list watch

- **Location**: `src/tui/task_list_watch.rs`, `src/commands/task.rs`
- **Description**: Implement polling clear-and-reprint task status screen, grouping by feature, icons/colors, proof state display, and `--interval`.
- **Dependencies**: Tasks 3.1, 3.3
- **Writes**: `src/tui/task_list_watch.rs`, `src/tui/mod.rs`, `src/commands/task.rs`
- **Acceptance Criteria**:
  - Layout matches section 2.5 and Appendix A.3/A.6.
  - Interval defaults to 2 seconds and clamps below 1 second.
  - Header counts distinct active agents.
- **Validation**: render snapshot tests with fixed terminal width and fixture tasks

### Task 6.2 - Migration check mode

- **Location**: `src/commands/migrate.rs`, `src/migrate/v0_106_to_v0_8.rs`
- **Description**: Implement `maestro migrate --check` as read-only migration planning from v0.106.1 artifacts to v0.8 shape.
- **Dependencies**: Phases 1-4
- **Writes**: `src/commands/migrate.rs`, `src/migrate/v0_106_to_v0_8.rs`, `src/migrate/mod.rs`
- **Acceptance Criteria**:
  - Reads old artifacts and prints unified diff.
  - Writes nothing in `--check`.
  - Covers mapping table in section 44.
- **Validation**: migration fixture tests asserting no writes and expected diff

### Task 6.3 - Migration apply mode

- **Location**: `src/migrate/v0_106_to_v0_8.rs`, `src/commands/migrate.rs`
- **Description**: Implement `maestro migrate` apply mode with backups, side-by-side safety, archive rules, stale proof reconstruction, and `--force` behavior.
- **Dependencies**: Task 6.2
- **Writes**: `src/migrate/v0_106_to_v0_8.rs`, `src/commands/migrate.rs`
- **Acceptance Criteria**:
  - Backups are created before writes.
  - Dropped concepts archive read-only.
  - Concurrent v0.106.1 writer evidence refuses without `--force`.
  - Verified proofs reconstruct only when commit + hashes match, otherwise stale.
- **Validation**: migration fixture tests for each artifact class and concurrent writer refusal

### Task 6.4 - Final V1 demo and release gate

- **Location**: `tests/e2e/v1_demo.rs`, repository docs if present
- **Description**: Add an end-to-end V1 dogfood demo matching section 42 and final release checks.
- **Dependencies**: Tasks 6.1, 6.3
- **Writes**: `tests/e2e/v1_demo.rs`
- **Acceptance Criteria**:
  - Fresh temp repo can run `init`, `install --agent claude`, `install --agent codex`, create/explore/accept/claim/complete/verify task, render watch output, metrics summary, query proof, and MCP smoke.
  - No dropped V1 systems appear in help output.
- **Validation**: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, E2E demo test

## Fixture Requirements

- `tests/fixtures/hook_payloads/`: Claude and Codex payloads for all six hook events, malformed payload, missing session id.
- `tests/fixtures/maestro_v0_106/`: old tasks JSONL, missions JSONL, evidence, verdicts, handoffs, plans, policies, workflows, intake, features, ADRs, skills.
- `tests/fixtures/artifacts_v0_8/`: golden `.maestro/` trees for init, task lifecycle, feature views, verification, metrics, and migration.
- `tests/fixtures/config_mirrors/`: CLAUDE.md, AGENTS.md, `.gitignore`, `.claude/settings.local.json`, `.codex/hooks.json`, `.codex/config.toml` with user content and managed blocks.
- `tests/fixtures/tui/`: task sets for each icon/proof/blocker combination.

## Phase Boundary Gates

Every phase must finish with:

1. `cargo fmt -- --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
4. Focused CLI smoke for the phase demo
5. Review-and-simplify gate on the phase diff
6. Review-swarm gate on the phase diff
7. One scoped Conventional Commit

## Final Acceptance Criteria

- `maestro init` creates only the V1 minimal artifact surface.
- `maestro install --agent claude|codex` is config-safe, reversible, and records install ownership.
- Hooks passively append privacy-preserving events and never block agent work.
- Task lifecycle, blockers, state_history, acceptance, and verification proof freshness work end to end.
- Feature views are implicit from task scan.
- Decision records use `decision-NNN-<slug>.md`.
- Four bundled skills extract and mirror by symlink.
- Metrics, matrix, friction, backlog, and proof queries compute on demand without SQLite/cache.
- MCP server exposes the 13 `maestro_*` tools over stdio.
- `task list --watch` renders the section 2.5 layout.
- Migration from v0.106.1 supports `--check` and apply with backups.
- Help output does not advertise dropped systems.
- Full test gate passes: `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

## Open Risks For Goal-Loop

- `rmcp` API details may require checking current crate docs before implementation.
- GitHub Releases update/checksum behavior may need an abstraction first, then real release wiring later.
- Hook payload shapes should be verified against current Claude and Codex hook docs before finalizing normalization.
- Migration fixtures need real v0.106.1 samples or faithful synthetic artifacts before Phase 6 can be trusted.
