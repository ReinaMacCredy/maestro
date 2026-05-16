# Phase 6 Done: Scenario Testing

**Status:** Phase 6 complete on 2026-05-16.

Per ADR-0019, the v2 release is gated on a behavioral pass against eight scenarios spanning project state × familiarity × workflow mode (2 × 2 × 2 = 8 cells). This phase authored the scenarios, built the runner, and executed the swarm-fix-loop until all eight passed in a single dispatch with no intervening commits.

---

## What landed

### PR 55: Author 8 scenarios + rubrics + agent briefs

- `adc91c97` `feat(scenarios): author 8 scenarios + rubrics + briefs (Phase 6 PR 55)`

Authored under `tests/scenarios/<name>/`:

| Scenario | Project | Familiarity | Mode |
|---|---|---|---|
| `greenfield-novice-light` | greenfield | novice | light |
| `greenfield-novice-heavy` | greenfield | novice | heavy |
| `greenfield-expert-light` | greenfield | expert | light |
| `greenfield-expert-heavy` | greenfield | expert | heavy |
| `brownfield-novice-light` | brownfield | novice | light |
| `brownfield-novice-heavy` | brownfield | novice | heavy |
| `brownfield-expert-light` | brownfield | expert | light |
| `brownfield-expert-heavy` | brownfield | expert | heavy |

Each directory contains a `scenario.md` (informational user-mock script + termination contract), a `rubric.ts` (deterministic check against `.maestro/evidence/*.jsonl`), and an `agent-brief.md` (the prompt handed to spawned sub-agents). Shared evidence-reading helpers live at `tests/scenarios/_helpers/rubric-helpers.ts`.

The scenario-authoring agent corrected several planner assumptions against code reality:
- `EvidenceRow` `kind` values are exactly `"transition"` and `"lint-violation"` (no `verify` or `principle-promote` row kinds).
- Plan states are `intake | specified | planned | in-progress | completed | cancelled`. There is no `draft` plan state. Heavy-mode rubrics check `plan-reached-specified` and `plan-reached-planned`.
- `task get <id>` does not exist as a v2 CLI verb (only as the MCP `maestro_task_get` tool). Agent briefs instruct sub-agents to read `.maestro/tasks/tasks.v2.jsonl` directly to inspect task state.
- `plan decompose <pln-id> --file <path>` (positional id, required `--file`) is the canonical heavy-mode decomposition verb.
- `maestro setup migrate-v2` is a subcommand, not a `--migrate-v2` flag. Brownfield rubrics check `.maestro/.migrated-v2.json` for migration success.
- `verify --verdict block` writes `verdict: "BLOCK"` (uppercase) on the transition row. Scenario 8's `task-blocked-with-verdict` rubric check matches uppercase.
- Architecture lint scope is `src/v2/**/*.ts` only. Scenario 4 creates its own `docs/architecture.yaml` in the sandbox project to deliberately trigger a `passive-harness` lint violation.

### PR 56: Rubric runner + swarm dispatcher

- `ecf9de4e` `feat(scenarios): rubric runner + swarm dispatcher (Phase 6 PR 56)`

Three scripts under `scripts/scenarios/`:
- `check.ts <scenario-name> <project-dir>` — single-scenario rubric runner. Imports `tests/scenarios/<name>/rubric.ts`, invokes `runRubric`, prints per-check PASS/FAIL trace, exits 0 on PASS.
- `check-all.ts` — reads `.maestro/scenarios/last-run.json` and runs each recorded scenario's rubric against its recorded sandbox path. Prints a summary table and exits 0 only if every scenario PASSes.
- `swarm.ts [--scenarios <subset> | --all]` — prepares fresh `mktemp -d` sandboxes for the requested scenarios. For greenfield: `git init -b main` + `maestro setup bootstrap`. For brownfield: `cp -R tests/fixtures/v1-maestro/.maestro` + `git init -b main`. Fills `<SANDBOX_PATH>` and `<MAESTRO_CHECKOUT>` placeholders in each scenario's `agent-brief.md` and writes the result to `<tmpdir>/.maestro/scenarios/filled-brief.md`. Writes the run map to `<repoRoot>/.maestro/scenarios/last-run.json`. Prints dispatch instructions for the operator's Claude Code session — does NOT spawn any sub-agents.

Per ADR-0019, the dispatcher is operator-driven: the actual Agent tool calls (with `run_in_background: true`) happen in the operator's Claude Code session, not in the script. The script's job is sandbox preparation and result aggregation.

### PR 57: Execute swarm-fix-loop until all 8 green

- `<plan-decompose-fix-sha>` `fix(v2/plan-decompose): include plan_id on child task draft evidence rows`
- `<phase-6-done-sha>` `docs(phase-6): mark Phase 6 done + swarm evidence`

#### Pass 1: 4 PASS, 4 FAIL

First swarm dispatch surfaced a single underlying bug across all four heavy-mode scenarios:

| Scenario | Result | Failed check |
|---|---|---|
| greenfield-novice-light | PASS | — |
| greenfield-novice-heavy | FAIL | `multiple-child-tasks-drafted` |
| greenfield-expert-light | PASS | — |
| greenfield-expert-heavy | FAIL | `multiple-child-tasks-drafted` |
| brownfield-novice-light | PASS | — |
| brownfield-novice-heavy | FAIL | `multiple-child-tasks-drafted` |
| brownfield-expert-light | PASS | — |
| brownfield-expert-heavy | FAIL | `multiple-child-tasks-drafted` |

**Root cause (category A — maestro bug):** `src/v2/service/plan-decompose.usecase.ts` lines 140-153 emitted child task draft evidence rows with only `task_id` set; the rubric `multiple-child-tasks-drafted` requires `plan_id` on those rows for traceability (so a viewer can group child tasks by their parent plan from the evidence trail alone). Four independent scenarios surfaced the same bug — exactly the signal Phase 6 was designed to catch before release.

**Fix:** Added `plan_id: plan.id` to the `emitTransitionEvidence` input at line 148. `TransitionEvidenceRow` already declared `plan_id?: string` alongside `task_id?: string`, so the type accepted the addition. The accompanying unit test `tests/unit/v2/service/plan-decompose.usecase.test.ts` was updated to assert that child task transitions now carry both fields and to filter for plan-only transitions via the `task_id === undefined` predicate.

After rebuilding (`bun run release:local`), re-prepped all 8 sandboxes via `bun scripts/scenarios/swarm.ts --all` and re-dispatched.

#### Pass 2 (final): 8/8 PASS, zero fixes in between

All 8 scenarios passed in a single dispatch with no commits between sandbox prep and `check-all.ts` result collection. This satisfies the master plan §10 done criterion: *"one full swarm pass completes with all 8 green and zero fixes in between."*

| Scenario | Exit status | Rubric | Evidence rows |
|---|---|---|---|
| greenfield-novice-light | pass | PASS | 5 |
| greenfield-novice-heavy | pass | PASS | 10 |
| greenfield-expert-light | pass | PASS | 5 |
| greenfield-expert-heavy | pass | PASS | 13 |
| brownfield-novice-light | pass | PASS | 6 |
| brownfield-novice-heavy | pass | PASS | 11 |
| brownfield-expert-light | pass | PASS | 6 |
| brownfield-expert-heavy | pass | PASS | 13 |

---

## Done criteria check

From master plan §10, Phase 6:

**8 scenarios + rubrics + fixtures authored:** DONE. `tests/scenarios/` contains 8 directories, each with `scenario.md`, `rubric.ts`, `agent-brief.md`. The `tests/fixtures/v1-maestro/` fixture is consumed by the four brownfield scenarios.

**Swarm dispatcher works:** DONE. `bun scripts/scenarios/swarm.ts --all` prepares 8 fresh sandboxes, writes `last-run.json`, and prints dispatch instructions in under 5 seconds.

**One full swarm pass with all 8 green and zero fixes in between:** DONE. Pass 2 (above) completed 8/8 PASS with no intervening commits to `src/`, `skills/`, `tests/scenarios/`, or `scripts/scenarios/`.

---

## Open follow-ups

These were surfaced during scenario execution but did NOT block Phase 6 closure. They are queued for Phase 7 cleanup or v2.1.

**1. `maestro evidence record --task <v2-id>` returns "Task not found"** (medium). The `evidence` CLI verb at `src/features/evidence/commands/evidence.command.ts:112` reads from v1 `services.taskStore`, not v2's task store. Sub-agents in scenarios 1, 3, 5 hit this when attempting ad-hoc evidence recording during the verify ritual; the failure was non-blocking because state-transition evidence is written automatically by the `claim`/`verify`/`ship` use cases (which use v2 stores). The `maestro-verify` SKILL.md still instructs agents to call `evidence record` for richer kinds (`ai-review`, `threat-model`, `manual-note`, etc.), so this surface needs to be either rewired to v2 or formally retired before v2.0 ships. Recommended: rewire to v2 in Phase 7.

**2. `setup bootstrap` does not seed `docs/architecture.yaml`** (low). External projects discover the file is required only at `task verify` time (when `task-verify.usecase.ts` looks for it). Phase 5's `06529ef5` made the missing file gracefully equivalent to zero violations, so the failure mode is now informational rather than blocking. A friendlier UX would have `setup bootstrap` seed a minimal `docs/architecture.yaml` stub or `setup check` warn when it's absent. Deferred to v2.1.

**3. `tests/fixtures/v1-maestro/README.md` layout-table drift** (informational). The fixture's README originally listed `handoffs/` and `plans/` directories that do not exist on disk. PR 55 corrected the layout table during scenario authoring. No further action.

---

## What's next

Phase 7: 2.0 release. Tag `v0.LAST` on main, cut `v2.0.0-rc.1` from `harness-os`, soak window (real 7 days or operator-override per ADR-0007 / Phase 7 plan path B), re-run swarm against the RC tag, external-project dogfood against the RC binary, then ship `v2.0.0` and fast-forward `harness-os` → `main`.
