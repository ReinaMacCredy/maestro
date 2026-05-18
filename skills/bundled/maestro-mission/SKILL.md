---
name: maestro-mission
description: Turn an approved heavy-mode product-spec into an executable mission with child tasks. Use after `maestro-design` has produced a `mode: heavy` spec, or when a single task has grown big enough that it should be decomposed into a multi-PR batch. Persists the mission to `.maestro/missions/missions.jsonl` and the child tasks to `.maestro/tasks/tasks.jsonl`.
---

# Maestro Mission

Heavy-mode work runs as a mission with child tasks. This skill is the bridge from a heavy spec to the first claimed task. Light-mode specs skip this skill — they go straight to `maestro-task`.

---

## When to use

- The spec was authored via `maestro-design` with `mode: heavy`.
- The work spans 3+ vertical slices, multiple feature dirs, or has dependency edges between phases.
- A single in-flight task has revealed too many surfaces; promote it.

Do not use for:

- Light-mode specs (one PR / one task).
- Read-only research or one-off corrections.

---

## Enter plan mode

Call `EnterPlanMode` as the first action of this skill, before reading context or drafting the plan.

Plan mode enforces read-only behavior so the plan is built from verified context without side effects. Stay in plan mode for the entire drafting pass: exploration, synthesis, phasing, validation design. Do not edit files or run destructive commands while in plan mode. Use Read, Grep, Glob, and Bash for inspection only.

When the plan is complete and ready to present, call `ExitPlanMode` with the full plan content as the argument. If the user rejects the plan, re-enter plan mode and revise rather than starting implementation.

---

## Ground on the spec and the real codebase

- Read the heavy-mode spec at `.maestro/specs/<slug>.md`. Carry forward `acceptance_criteria`, `non_goals`, `work_type`, and any open questions.
- Inspect the actual artifacts the spec touches. Read enough of the current system to plan against reality, not assumptions.
- Mark assumptions clearly when information is missing.

If the spec is light-mode or has not been authored, stop and route the user to `maestro-design` first.

---

## Build a phased plan

- Organize the work into 2 to 7 phases for non-trivial scope.
- Give each phase a clear outcome, not just an activity label.
- Sequence phases by dependency order.
- Make tasks outcome-named, concrete, and falsifiable.
- Keep each task small enough to ship as one PR (ADR-0006: 1:1 task↔PR).

For each phase, include: purpose, child tasks, dependencies, verification checkpoint.

---

## Attach validation to the plan

- Define the smallest relevant check for each phase.
- Call out tests, lint, type checks, builds, manual verification, or rollout checks.
- Prefer the cheapest check that can falsify the phase.
- If implementation should be test-first, identify the test surfaces up front.

The verification protocol is `maestro-verify` — every task ends with `maestro task verify`, every PR is gated by CI Maestro.

---

## Surface risks and cut lines

- Identify the highest-risk assumption or dependency.
- Separate must-have work from optional polish.
- State what can be deferred without undermining the goal.

---

## Materialize the mission and child tasks

When the user approves, run the mission lifecycle. There are four entry points; pick whichever matches the starting material:

### A. From a heavy-mode spec (most common path)

```bash
maestro mission new "<title>" --from-spec .maestro/specs/<slug>.md
```

Creates a mission in `approved`. `approved` here means "spec parsed", NOT a human approval gate. Captures the returned `pln-...` id. Requires `mode: heavy` on the spec — light-mode specs are rejected with `MissionRequiresHeavyModeError`.

### B. From a JSON task batch

```bash
maestro mission new "<title>" --from-file tasks.json
```

Reads a JSON array of `{title, slug, spec_path?}` task objects and creates a mission directly in `planned` with those tasks seeded as drafts. Skips the `approved` step entirely.

### C. From a template

```bash
maestro mission new "<title>" --template refactor    # or: feature, bug, migration
```

Built-in templates seed 4 starter tasks (slugged under the mission slug) and land at `planned`. Custom templates live in `.maestro/templates/missions/<name>.yaml` and override built-ins of the same name. Run `maestro mission new --list-templates` to discover available templates.

### D. Bare title, decompose later

```bash
maestro mission new "<title>"                                    # creates at intake
maestro mission decompose <pln-id> --file tasks.json             # advances intake -> planned
```

Bare missions sit in `intake` until decompose attaches tasks. `mission decompose` accepts both `intake` and `approved` as input states, so spec-first and stub-first workflows both work.

### Decompose batch shape

```json
[
  { "title": "Scaffold feature X",           "slug": "scaffold-feature-x" },
  { "title": "Implement core use-case",      "slug": "impl-feature-x-core" },
  { "title": "Add unit + integration tests", "slug": "test-feature-x" }
]
```

The batch is validated atomically: no duplicate slugs within the batch, no collisions with existing tasks. Decompose refuses missions that already have any tasks — use `task from-spec` to add more tasks manually, or `mission cancel` and start over.

### Inspect the mission

```bash
maestro mission show <pln-id>
maestro mission show <pln-id> --json
```

Prints the mission state, the spec path, and every child task with its current state.

### Start work

The mission auto-advances `planned → in-progress` on the first task claim (ADR-0011). After that, mission state auto-derives from task rollup — no manual `mission start` or `mission complete` verb. Load `maestro-task` and execute from there.

---

## Mission state machine (reference)

```
                                                        +- (auto-pause) -> paused
                                                       /                       \
intake -> approved -> planned -> in-progress ---------+                         +-> in-progress (auto-resume)
   \         \           \           \                 \                       /
    \         \           \           \                 +- (rollup) -> completed | failed
     \         \           \           \
      \         \           \           +-> cancelled (verb: mission cancel)
       \         \           +-> cancelled
        \         +-> cancelled
         +-> cancelled
```

- `intake → planned` is the bare-title decompose path.
- `approved` is the spec-parsed state, not a reviewer gate (gates are task-level).
- `paused` fires automatically when every active task is blocked; `in-progress` resumes when any unblocks.
- `completed` requires every task `shipped`; any `abandoned` task flips the terminal to `failed`.
- `completed`, `failed`, `cancelled` are absorbing. To "retry" a failed mission, create a new one.

---

## Cancel a mission

```bash
maestro mission cancel <pln-id>
maestro mission cancel <pln-id> --reason "<text>"
```

Cascades to active tasks (transitions them to `abandoned`) then transitions the mission to `cancelled`. Idempotent on already-cancelled missions. Errors on `completed` / `failed` — those represent different outcomes and shouldn't be re-stamped. Best-effort: a task that fails to abandon mid-cascade is reported in the result (`cascadeErrors`), and the mission still cancels.

---

## Persist a human-readable mission note (optional)

The mission record in `.maestro/missions/missions.jsonl` carries `title`, `slug`, `spec_path`, and `state` — it does not carry narrative. For non-trivial missions, persist a markdown sidecar so future sessions and reviewers have the rationale:

`.maestro/missions/<slug>.md`:

```markdown
# <Title>

## Objective
<1-2 sentence restatement of the heavy-mode spec's goal>

## Scope
**In:** <what is included>
**Out:** <what is explicitly excluded>

## Research findings
<context gathered during planning, grounded in verified files or artifacts>

## Phases
- [ ] Phase 1 — <outcome> — tasks: <tsk-...>, <tsk-...>
- [ ] Phase 2 — <outcome> — tasks: <tsk-...>

## Verification
<commands, tests, manual checks per phase>

## Risks and cut lines
<the highest-risk assumption; what can be deferred>
```

Collision handling: if `.maestro/missions/<slug>.md` already exists, append a numeric suffix (`<slug>-2.md`).

Skip this section entirely when no maestro project is detected.

---

## What this skill does not do

- Does **not** propose a contract. Contracts are derived from the spec's path globs and the task's diff; the agent does not author them in the plan.
- Does **not** lock a risk class. Maestro derives it from the diff; you cannot lower it from the plan.
- Does **not** open PRs. Each child task ends with `maestro task ship`; the PR opens through normal git flow before `ship`.
- Does **not** schedule iteration. Maestro stays passive (no cron, no daemon).

---

## Hand off cleanly

The next phase after this skill is `maestro-task`.
Pass a decomposed mission with child tasks materialized — not just a mission record.
Do not invoke implementation from this skill.

When the child tasks exist and the user approves, invoke the `Skill` tool with `skill: "maestro-task"` and claim the first child. `mission decompose` does **not** emit a handoff envelope; the only handoff trigger here is the first `maestro task claim`, which writes a `task:claim` envelope to `.maestro/handoffs/<hnd-...>.json` (see `maestro-handoff` for the read side).

---

## See also

- `maestro-design` — grill-protocol spec authoring (run before this skill).
- `maestro-task` — single-task execution loop (run after this skill for each child).
- `maestro-verify` — canonical verification protocol.
- `docs/cli-reference.md` — verb-by-verb reference.
- `docs/adr/0003-task-lifecycle.md`, `docs/adr/0011-exec-plan-auto-complete.md` — the lifecycle decisions this skill rides on.
