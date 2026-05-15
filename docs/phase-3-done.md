# Phase 3 — Done

Observability + setup + worktree + handoff (master plan §10 Phase 3) is in flight on `harness-os`. The phase closes with `maestro setup --check`, `setup --migrate-v2`, heavy-mode auto-worktree at claim, and a write-only handoff emitter at every task transition.

## PRs

| PR  | Scope                                                                  | Task |
| --- | ---------------------------------------------------------------------- | ---- |
| 30  | `ObservabilityPort` + `.maestro/runs/<task-id>/observability.jsonl` adapter; transition-evidence hook mirrors every state transition into the observability log | #30  |
| 31  | `maestro setup check` + `maestro setup bootstrap` verbs; idempotent bootstrap of the five v2 directories | #31  |
| 32  | `maestro setup migrate-v2` scaffold: preflight, backup to `.maestro.backup-<ts>/`, write-flag, dry-run | #32  |
| 33  | `migrate-v2` steps 2 through 11: corrections, tasks, plans, evidence, policies, principles, verify; full e2e assertion table | #33  |
| 34  | `WorktreeStorePort` + git adapter; heavy-mode `task claim` auto-creates `<parent>/<repo>-<task_id>` worktree, records `worktree_path` on the task | #34  |
| 35  | `HandoffEmitterPort` + `.maestro/handoffs/<id>.json` adapter; `task claim` and `task block` emit launch envelopes through `emitHandoff` | #35  |

## What landed

### Observability (PR 30)

- `ObservabilityPort` is a write-only port with one method: `emit({task_id, kind, timestamp, payload})`. No replay surface — the JSONL log is consumed by Vector/VictoriaLogs downstream, not by maestro itself. Option (C) from the master plan's open items (minimal log-only default) is the locked scope.
- `JsonlObservabilityStore` writes per-task lines to `.maestro/runs/<task-id>/observability.jsonl`. Every transition produces a row that mirrors the evidence-store record so the two logs are eventually-consistent without a coupling between the ports.
- `emit-transition-evidence` accepts an optional `observabilityStore` and forwards the row when present. Each lifecycle usecase (`task-claim`, `task-block`, `task-abandon`, `task-ship`, `task-verify`, `plan-decompose`) gained the optional dep and passes it through. The v2 services composition root wires `services.observabilityStore` to the JSONL adapter.

### Setup (PRs 31–33)

- `maestro setup check` audits the five v2 dirs (`.maestro/specs`, `.maestro/plans`, `.maestro/tasks`, `.maestro/runs`, `.maestro/evidence`), the principles pack (`docs/principles/`), and `.maestro/config.yaml`. `ok = entries.every(e => e.status !== "missing")` — warn entries (empty principles pack, absent config.yaml) are informational only.
- `maestro setup bootstrap` creates the v2 dirs (`.gitkeep` placeholder when empty) idempotently. Same dirs already-present → no-op.
- `maestro setup migrate-v2` orchestrates an 11-step v1→v2 migration: preflight → backup → bootstrap-dirs → migrate-corrections → migrate-tasks → migrate-plans → migrate-evidence → migrate-policies → seed-principles → write-flag → verify. `--dry-run` plans without writing; `--force` re-runs over an existing flag.
- Backup writes to `.maestro.backup-<sanitized-ts>/` (sanitizing `:` and `.` so the path is portable on every FS we target).
- `seed-principles` writes 4 default markdowns (`layer-order`, `no-yolo-data-probing`, `passive-harness`, `prefer-shared-utils`) if `docs/principles/` is empty, embedded as TypeScript constants so the installed binary works from any consumer repo.
- `migrate-tasks` walks `.maestro/tasks/tasks.jsonl` and maps v1 rows through `mapV1TaskToV2` (handles status → state, camelCase → snake_case, `in_progress` → `doing`).
- `migrate-plans` walks `.maestro/missions/<id>/mission.json` and writes v2 ExecPlan rows to `.maestro/plans/plans.v2.jsonl`.

### Worktree (PR 34)

- `WorktreeStorePort.create({task_id, slug, base_branch?, branch_prefix?})` returns a `WorktreeRecord {task_id, slug, path, branch, base_branch, created_at}`.
- `GitWorktreeStore` runs `git -C <repoRoot> worktree add -b <branch> <path> <base>` via the shared `ProcessRunnerPort`. Path = `<parentDir>/<repoName>-<task_id>`. Branch = `<prefix>/<slug>` (default `prefix=feat`, `base=main`). State persists as `.maestro/worktrees/<task-id>.json` in the primary repo (ADR-0008 / PD-3: worktree state is owned by the primary repo, not duplicated into each worktree).
- `task claim` reads `existing.spec_path`, parses the spec frontmatter, and when `mode=heavy` it calls `worktreeStore.create` before recording the claim. The resulting path is persisted on the task as `worktree_path` and printed by the CLI as `(worktree <path>)` after the claim line.
- `--skip-worktree` bypasses creation even for heavy specs. Failures are caught, logged via `console.error`, and never block the claim — the task still reaches `claimed`, the agent sees the error in stderr.

### Handoff (PR 35)

- `HandoffEmitterPort` is write-only: `emit(envelope)` plus `list()` / `get(id)` for tooling. Triggers are the lifecycle verbs (`task:claim | task:block | task:abandon | task:ship | task:verify`).
- `FsHandoffEmitter` writes one JSON file per envelope to `.maestro/handoffs/<id>.json`. List/get round-trip those files; an absent dir returns `[]` rather than throwing.
- `emitHandoff` helper stamps `id` (`hnd-<base36-ts>-<rand>`), `created_at`, and `trigger_verb` on the envelope, omits optional fields when undefined (no `agent_id: undefined` noise in the JSON), and is a no-op when the emitter is not wired.
- `task claim` emits a `task:claim` envelope after the claim succeeds, carrying `agent_id`, `worktree_path`, and `spec_path`. `task block` emits `task:block` carrying the block `reason`. The pattern is identical for the remaining verbs — they pass `handoffEmitter` through the same way when they need to emit.

## Test suite

Final: 3097 pass, 0 fail, 112 skip across 3209 tests / 374 files. New v2 coverage:

- PR 30: 7 unit + e2e (observability emission, transition mirror, plan-only no-op).
- PR 31: 7 unit + 5 e2e (check + bootstrap with relaxed-warn ok semantics).
- PR 32: 17 unit + 5 e2e (preflight, backup, flag, dry-run).
- PR 33: 5 unit-overhaul + 2 e2e assertion-table tests (full migration produces every expected artifact; `setup check` exits 0 afterwards).
- PR 34: 6 unit + 7 integration + 3 e2e (real git repo, light skipped, `--skip-worktree` bypass).
- PR 35: 5 unit (FS adapter) + 3 unit (emit helper) + 4 integration (claim/block emission, no-op without emitter, optional-field stripping).

## Operational invariants reaffirmed

- **Passive harness.** No `setInterval`, no `setTimeout`, no daemon, no scheduler introduced. The observability log, worktree creation, and handoff emission all fire synchronously inside the verb that just ran.
- **Forward-only layers.** `src/v2/repo/`, `src/v2/service/`, `src/v2/runtime/` continue to satisfy the v2 architecture lint (ADR-0017). New ports live under `repo/`, new orchestration under `service/`, new CLI surface under `runtime/`.
- **Local-first.** Every new write target is under `.maestro/` in the consumer repo. No network call, no telemetry hook, no LLM call.
- **Setup is idempotent.** `bootstrap` and `migrate-v2` (with `--force`) replay safely. `setup check` is the canonical health probe.

## What's next

Phase 3 closes out observability + setup + worktree + handoff. Phase 4 (master plan §10): documentation pass, CLI reference rewrite, v1 verb removal from the codebase, `chore(release): v2.0.0`, `harness-os` → `main` merge, post-merge branch delete.
