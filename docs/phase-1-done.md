# Phase 1 done capture

Phase 1 of `docs/v2-master-plan.md` is feature-complete. This document
captures the dogfood run that satisfies the Phase 1 done criterion:

> dogfooded on a real maestro change, spec → task → ship, with transition
> evidence recorded, and `maestro task verify` runs the architecture
> lints as one of its checks.

## What landed

- PR 01 – PR 10: spine + state types + ports + adapters + spec/task usecases.
- PR 11: `ArchitectureRules` port + YAML adapter (`docs/architecture.yaml`).
- PR 12: architecture-lint runner (`runArchitectureLints`).
- PR 13: `task verify` (lint-driven PASS/FAIL) + `task ship` with hot-path
  aliases (`verify`, `ship`).
- PR 14: standalone `lint:arch` script (`bun run lint:arch`) that loads
  `docs/architecture.yaml` and runs the v2 lint runner.
- PR 15: `maestro-design` bundled skill (grill protocol from ADR-0016).
- PR 16: v1→v2 state-mapping functions + 10-row fixture test.
- PR 17: this capture + AGENTS.md "Maestro v2 (in flight)" section.

## ADRs that constrain Phase 1

- ADR-0003 two-lifecycle model.
- ADR-0004 hybrid transition triggers.
- ADR-0005 ports + default adapters.
- ADR-0007 big-bang v2 release.
- ADR-0011 auto-complete plan on all terminal.
- ADR-0014 verb naming (noun-verb with hot-path aliases).
- ADR-0016 grill protocol baked into design + plan skills.
- ADR-0017 cross-cutting layers are universally importable.

## Dogfood run

- **Spec:** `.maestro/specs/phase-1-done-capture.md`.
- **Task:** `tsk-mp6x70og-zflvrp`.
- **Date:** 2026-05-15.
- **Agent:** `claude-opus-4-7`.

The dogfood used only v2 verbs (`spec new`, `spec validate`,
`task from-spec`, `task claim`, `task verify`, `task ship`). The
five transition rows in `.maestro/evidence/2026-05-15.jsonl`
tagged with this task's id are:

| # | id                       | from → to            | trigger          | verdict |
|---|--------------------------|----------------------|------------------|---------|
| 1 | `evd-mp6x70oh-nk4d9q`    | null → draft         | `task:from-spec` |         |
| 2 | `evd-mp6x7474-xuos3h`    | draft → claimed      | `task:claim`     |         |
| 3 | `evd-mp6x80mg-wd4wdo`    | claimed → verifying  | `task:verify`    |         |
| 4 | `evd-mp6x80mz-qgwtv7`    | verifying → ready    | `task:verify`    | PASS    |
| 5 | `evd-mp6x84xg-019wtu`    | ready → shipped      | `task:ship`      | PASS    |

`maestro task verify` ran the architecture-lint usecase against
`src/v2/**/*.ts` and reported **0 violations across 34 files**
(`bun run lint:arch` returns the same scan stats).

## Where to look next

- `docs/v2-master-plan.md` for Phase 1.5 (principles + correction-recording
  bridge) and beyond.
- `docs/architecture.yaml` for the layered rules `task verify` enforces.
- `skills/bundled/maestro-design/SKILL.md` for the grill protocol used
  to author new specs.
