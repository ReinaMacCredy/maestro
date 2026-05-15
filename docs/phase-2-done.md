# Phase 2 — Done

Heavy-mode exec-plan lifecycle (ADR-0003 / ADR-0011) is in flight on `harness-os`.

## PRs

| PR  | Scope                                                                 | Task |
| --- | --------------------------------------------------------------------- | ---- |
| 18  | ExecPlan type + store port + JSONL adapter; `plan_id` on Task         | #19  |
| 19  | `plan from-spec` + `plan show` CLI verbs                              | #20  |
| 20  | `plan decompose` reads task batch and creates child tasks             | #21  |
| 21  | `tryAdvancePlan` helper wired into task-claim/ship/abandon            | #22  |
| 22  | `task verify --verdict {human,block} --reason` + exit codes 2/3       | #23  |
| 23  | maestro-plan + maestro-verify SKILL.md updates + this dogfood capture | #24  |

## What landed

- **ExecPlan lifecycle.** `intake -> specified -> planned -> in-progress -> completed` with `cancelled` as the only manual terminal. Pure transition table in `src/v2/types/exec-plan-state.ts`; assertion helper mirrors the task-state shape.
- **Storage.** `.maestro/plans/plans.v2.jsonl` is the append-only authoritative log; `JsonlExecPlanStore` uses the same FIFO mutation queue as `JsonlTaskStore`.
- **Plan verbs.** `plan from-spec <path>`, `plan decompose <id> --file <path|->`, `plan show <id> [--json]`. No hot-path aliases — plan verbs are heavy and not on the loop critical path.
- **Auto-advance (ADR-0011).** `task claim` advances `planned -> in-progress` on first child claim; `task ship`/`task abandon` advance `in-progress -> completed` when every child is terminal. Idempotent and gracefully handles the edge case where every child is abandoned before the plan ever sees a claim.
- **Explicit verdicts on `task verify`.** `--verdict human --reason <text>` stays at `verifying` and exits 2; `--verdict block --reason <text>` transitions to `blocked` and exits 3. Default PASS/FAIL unchanged. Missing `--reason` exits 1.
- **Skill updates.** `maestro-plan` now documents the v2 heavy-mode handoff (`plan from-spec` -> `plan decompose --file -`) alongside the v1 batch path. `maestro-verify` documents the 4-exit-code routing including HUMAN/BLOCK from `task verify`.

## Test suite

Final: 2952 pass, 0 fail, 112 skip across 3064 tests / 351 files. New v2 coverage adds 21 unit tests and 12 e2e tests across the three new use cases and the verify-verdict extension.

## Dogfood capture (real artifacts on this branch)

Spec: `.maestro/specs/phase-2-dogfood.md` (heavy mode).

Plan id: `pln-mp6zpjh2-5gijnm`. Title: `phase-2-dogfood`.

Three child tasks created via `plan decompose`:

| Task                   | Slug                     | Terminal state | Path                                            |
| ---------------------- | ------------------------ | -------------- | ----------------------------------------------- |
| `tsk-mp6zpurn-u5u8c1`  | `phase2-dogfood-slice-a` | `shipped`      | claim -> verify PASS -> ship                    |
| `tsk-mp6zpuro-oupbmt`  | `phase2-dogfood-slice-b` | `abandoned`    | claim -> verify HUMAN -> abandon                |
| `tsk-mp6zpuro-4nsjmj`  | `phase2-dogfood-slice-c` | `abandoned`    | claim -> verify BLOCK -> abandon                |

Evidence rows captured in `.maestro/evidence/2026-05-15.jsonl`:

| Evidence id              | Plan/Task                           | Trigger              | Verdict | Notes                              |
| ------------------------ | ----------------------------------- | -------------------- | ------- | ---------------------------------- |
| `evd-mp6zpjh2-id2ywk`    | `pln-mp6zpjh2-5gijnm`               | `plan:from-spec`     |         | `null -> specified`                |
| `evd-mp6zpurp-nijckz`    | `pln-mp6zpjh2-5gijnm`               | `plan:decompose`     |         | `specified -> planned`             |
| `evd-mp6zq2nr-4hfgym`    | `pln-mp6zpjh2-5gijnm`               | `plan:auto-start`    |         | `planned -> in-progress`           |
| `evd-mp6zq83g-6pz3s1`    | slice A                              | `task:verify`        | `PASS`  | `verifying -> ready`               |
| `evd-mp6zqi18-6yki6k`    | slice B                              | `task:verify`        | `HUMAN` | self-loop at `verifying`, reason recorded |
| `evd-mp6zqnla-xhzgsa`    | slice C                              | `task:verify`        | `BLOCK` | `verifying -> blocked`, reason recorded |
| `evd-mp6zqyv0-xgiqv0`    | `pln-mp6zpjh2-5gijnm`               | `plan:auto-complete` |         | `in-progress -> completed`         |

The full chain (`plan from-spec` -> `plan decompose` -> `claim` -> `verify` -> `ship`/`abandon` -> auto-complete) executed against the installed binary with no manual intervention beyond the verbs themselves.

## What's next

Phase 2 closes out the heavy-mode loop. Phase 1.5 (correction-recording bridge) and Phase 3 (skill bundle / autopilot interplay) are the remaining tracks per `docs/v2-master-plan.md`.
