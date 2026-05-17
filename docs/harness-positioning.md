# Harness positioning

Maestro v2 is **the harness OS for agent-generated codebases**. Humans steer. Agents execute. Maestro is the substrate — local-first, verb-driven, passive. Host runtimes (Claude Code, Codex, Cursor, CI) call maestro verbs; maestro never schedules, daemonizes, or runs an LLM itself.

This document maps the v2 primitives to the source locations that implement them. For the full verb surface, see `docs/cli-reference.md`. For the locked architectural decisions, see `docs/adr/`.

---

## The two primitive families

Maestro v2 has four **knowledge primitives** (directories the agent reads at session start) and four **execution primitives** (the agent's verb-shaped surface). Everything else is either composed from these or is non-goal for v2.0.

### Knowledge primitives

| Primitive       | Source of truth                                  | What lives here                                                                |
| --------------- | ------------------------------------------------ | ------------------------------------------------------------------------------ |
| `product-specs` | `.maestro/specs/<slug>.md`                       | Feature specifications with acceptance criteria + non-goals + work-type.       |
| `architecture`  | `docs/architecture.yaml` + `src/service/architecture-lint.usecase.ts` | Mechanically-enforced layering and dependency rules. |
| `quality-score` | `.maestro/quality-score.json` (`gc grade`)       | Per-domain grade tracking gaps over time.                                      |
| `principles`    | `docs/principles/*.md`                           | Named golden rules with `Rule | Rationale | Scan Command | Fix Recipe`.        |

The agent reads these; maestro writes them only on explicit verbs (`spec new`, `principle promote`, `mission from-spec`, etc.).

### Execution primitives

| Primitive  | One-line role                                                                                                 |
| ---------- | ------------------------------------------------------------------------------------------------------------- |
| `worktree` | Isolated execution environment per task. Per-worktree `.maestro/runs/`, scoped telemetry and logs.            |
| `loop`     | Default execution mode after a task is claimed. Try → verify → evidence → iterate until PASS or stuck.        |
| `task`     | A unit of PR-shaped work with a lifecycle. Strictly 1:1 task↔PR (ADR-0006).                                   |
| `handoff`  | Artifact emitted at every state transition, carrying context for the next agent session.                      |

---

## Primitive → source map

### `product-specs`

- `src/repo/spec-store.port.ts` — port + types (mode, work_type, acceptance criteria, non-goals).
- `src/repo/fs-spec-store.adapter.ts` — filesystem reader with frontmatter parsing.
- `src/service/spec-new.usecase.ts`, `spec-validate.usecase.ts` — author + lint.
- `src/runtime/spec.command.ts` — `maestro spec new`, `maestro spec validate`.
- Authored interactively via the `maestro-design` SKILL.md grill protocol.

### `missions`

- `src/types/mission.ts`, `mission-state.ts` — 8-state machine `intake → approved → planned → in-progress ↔ paused → completed | failed | cancelled` (ADR-0011).
- `src/repo/mission-store.port.ts`, `jsonl-mission-store.adapter.ts` — append-only log at `.maestro/missions/missions.jsonl`.
- `src/service/mission-new.usecase.ts`, `mission-from-spec.usecase.ts`, `mission-show.usecase.ts`, `mission-decompose.usecase.ts`, `mission-cancel.usecase.ts`, `try-advance-mission.usecase.ts` — verbs + auto-rollup helper.
- `src/runtime/mission.command.ts` — `maestro mission new | from-spec | decompose | cancel | show`.

### `principles`

- `src/types/principle.ts`, `principles-store.port.ts` — schema for the 4-section principle markdown.
- `src/repo/fs-principles-store.adapter.ts` — filesystem reader for `docs/principles/*.md`.
- `src/service/principle-scan.usecase.ts` — runs each rule's `Scan Command` ripgrep and reports violations.
- `src/service/principle-promote.usecase.ts` — materialize a principle from a lint-violation evidence row.
- `src/service/default-principles.ts` — 4 default principle bodies embedded as TypeScript constants for the `setup` seed step.
- `src/runtime/principle.command.ts` — `maestro principle promote`.

### `architecture`

- `src/service/architecture-lint.usecase.ts` — file-scan rules (`forward-only-layers`, `no-cross-feature-deep-imports`, `no-runner-inversion`, others).
- Wired into `task verify` as one of the architecture checks.

### `task` (lifecycle)

- `src/types/task.ts`, `task-state.ts` — state machine `draft → claimed → doing ↔ verifying ↔ blocked → ready → shipped | abandoned` (ADR-0003).
- `src/repo/task-store.port.ts`, `jsonl-task-store.adapter.ts` — append-only log at `.maestro/tasks/tasks.jsonl`.
- `src/service/task-{from-spec,claim,block,abandon,ship,verify}.usecase.ts` — the six lifecycle verbs.
- `src/service/emit-transition-evidence.ts` — shared transition-evidence emitter (single emit point, mirrored into observability + handoff).
- `src/runtime/task.command.ts` — `maestro task *` + hot-path aliases `claim | verify | block | abandon | ship`.

### `worktree`

- `src/repo/worktree-store.port.ts` — port + record shape (`task_id`, `slug`, `path`, `branch`, `base_branch`, `created_at`).
- `src/repo/git-worktree-store.adapter.ts` — runs `git worktree add -b <branch> <path> <base>` via `ProcessRunnerPort`. State persists at `.maestro/worktrees/<task-id>.json` on the primary repo (PD-3 / ADR-0008).
- `task claim` auto-creates a worktree when the spec is `mode: heavy`; `--skip-worktree` opts out. Failures are logged but never block the claim.

### `handoff`

- `src/repo/handoff-emitter.port.ts` — write-only port; the trigger vocabulary is `task:claim | task:block | task:abandon | task:ship | task:verify`.
- `src/repo/fs-handoff-emitter.adapter.ts` — writes one JSON envelope per emission to `.maestro/handoffs/<id>.json`, plus an exclusive-create pickup sidecar at `.maestro/handoffs/<id>.picked_up.json`.
- `src/service/emit-handoff.ts` — stamps id + timestamp + trigger verb; omits optional fields when undefined; no-op when the emitter is not wired.
- `task claim` and `task block` call `emitHandoff` after the state transition lands. The other three triggers (`abandon`, `ship`, `verify`) are reserved in the port vocabulary; envelope emission for those is roadmap.

### Observability (per-task)

- `src/repo/observability.port.ts` — write-only port: `emit({task_id, kind, timestamp, payload})`.
- `src/repo/jsonl-observability.adapter.ts` — writes `.maestro/runs/<task-id>/observability.jsonl`.
- `emit-transition-evidence.ts` mirrors every transition into the observability log in addition to the evidence log; the two logs are eventually-consistent without coupling. Option C from the master plan (minimal log-only default) is the locked Phase 3 scope.

### Setup + migration

- `src/service/setup-check.usecase.ts` — audits the five v2 directories, principles pack, and config.
- `src/service/setup-bootstrap.usecase.ts` — idempotent dir creation with `.gitkeep`.
- `src/service/migrate-v2.usecase.ts` + `migrate-v2-steps.ts` — 11-step v1→v2 migration.
- `src/runtime/setup.command.ts` — `maestro setup check | bootstrap | migrate-v2 | migrate-corrections`.

### Loop primitive

The loop primitive is not a verb. It is the agent's behavior between `task claim` and a terminal `verify PASS`: try → `task verify` → read evidence → iterate. Convergence is detected by stable findings hashes recorded in the evidence log; the agent decides when to stop.

---

## External triggers

Maestro never schedules itself. Anything that needs to run on a clock lives outside the binary and calls maestro verbs as a subprocess. The three canonical shapes:

1. **GitHub Actions cron** — nightly `maestro gc doc-gardening --json`, weekly `maestro gc slop-cleanup`, etc.
2. **Host-runtime session hooks** — `.claude/settings.json`, `.codex/settings.json`, `.cursor/settings.json` `SessionStart` / `SessionEnd` hooks call maestro verbs at the start and end of an interactive session.
3. **Agent skill prompts** — a local skill instructs `maestro task verify <id>` after a substantive edit batch. Timing is contextual; the skill decides, not a scheduler.

---

## What this is not

Maestro deliberately is **not**:

- **A scheduler.** No cron, no daemon, no background process inside maestro. Scheduling lives outside maestro.
- **A daemon.** The binary runs only when invoked and exits.
- **An LLM client.** Maestro never makes model API calls; agents do, and call maestro verbs in between.
- **A background process.** No watcher, no filesystem poller, no long-lived state machine.

The structural guarantee is the `no-runner-inversion` rule in `src/service/architecture-lint.usecase.ts`: maestro code may not invoke schedulers or spin up persistent loops. The lint enforces it at `error` severity.

---

## Vocabulary disappearances on v2 (no aliases)

v1 `mission` (and the brief `exec-plan` rename) → v2 `mission` (post-v0.101 rename) · `spec` (old) → `product-spec` · `intake` / `brainstorm` → folded into `design-docs` reading + `product-spec` authoring · `session` / `notes` → folded into `handoff` (session-detect absorbed into `worktree` metadata).

Three v1 feature dirs disappear because their job is now done by knowledge primitives the agent reads at session start: `memory` + `memory-ratchet` + `agent` (corrections live in `docs/principles/`, agent prompt collapses into `AGENTS.md`); `graph` (project-to-project edges) folded into `~/.maestro/graph/projects.json`; `session` → notes folded into handoff, detect folded into worktree.

---

## Where to read next

- Verb surface: `docs/cli-reference.md`
- Decision register: `docs/adr/`
- Witness ladder: `docs/witness-levels.md`
- Risk derivation: `docs/risk-class-derivation.md`
- Policy format: `docs/policy-format.md`
- CI integration: `docs/ci-integration.md`
