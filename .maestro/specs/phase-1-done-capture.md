---
slug: phase-1-done-capture
acceptance_criteria:
  - docs/phase-1-done.md exists and captures the dogfood evidence (task id, transition evidence row ids, lint scan stats)
  - AGENTS.md has a "Maestro v2 (in flight)" section pointing readers to docs/v2-master-plan.md and the v2 verbs (spec, task from-spec/claim/verify/ship, lint:arch)
  - maestro task verify on this task PASSes (architecture-lint runner returns 0 violations across src/v2/**/*.ts)
non_goals:
  - Rewriting v1 sections of AGENTS.md (v1 verbs stay documented until Phase 4 deletion)
  - Wiring AGENTS.md generation into init-deep
  - Authoring docs/phase-1-done.md from spec content (the agent writes the report alongside the spec)
risk_class: low
mode: light
work_type: change-request
---

# Phase 1 done capture

## Context

Phase 1 of the v2 master plan is feature-complete on `main` (the harness-os
branch was inlined; we're committing v2 spine directly). PRs 11–16 landed the
ArchitectureRules port, the lint runner, `task verify` + `task ship` with
hot-path aliases, a standalone `lint:arch` script, the `maestro-design` skill
with the grill protocol, and v1→v2 state-mapping functions. The remaining
Phase 1 done criterion is: "dogfooded on a real maestro change, spec → task →
ship, with transition evidence recorded, and `maestro task verify` runs the
architecture lints as one of its checks."

This spec IS that dogfood. The "real change" is the Phase 1 done capture
itself: an AGENTS.md addition pointing at v2 + a docs/phase-1-done.md report
linking back to this spec's evidence rows.

## Decisions

- Use the existing v2 verbs only (no shortcut: no manual JSONL edits).
- Capture the transition evidence row ids and the verify lint stats verbatim
  in `docs/phase-1-done.md`.
- Keep AGENTS.md edits surgical: one new section near the top called
  "Maestro v2 (in flight)" with three bullets — what's done, what verbs are
  live, and where to read the plan. v1 sections stay untouched.
