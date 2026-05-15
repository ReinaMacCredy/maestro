---
name: maestro-plan
description: Turn an approved heavy-mode product-spec into an executable v2 exec-plan with child tasks. Use after `maestro-design` has produced a `mode: heavy` spec, or when a single task has grown big enough that it should be decomposed into a multi-PR batch. Persists the plan to `.maestro/plans/plans.v2.jsonl` and the child tasks to `.maestro/tasks/tasks.v2.jsonl`.
---

# Maestro Plan

Heavy-mode work runs as an exec-plan with child tasks. This skill is the bridge from a heavy spec to the first claimed task. Light-mode specs skip this skill — they go straight to `maestro-task`.

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

## Materialize the plan and child tasks

When the user approves, run the v2 plan lifecycle:

### 1. Promote the spec to an exec-plan

```bash
maestro plan from-spec .maestro/specs/<slug>.md
```

Creates an `ExecPlan` in `specified`. Captures the returned `pln-...` id. Requires `mode: heavy` on the spec — light-mode specs are rejected with `PlanRequiresHeavyModeError`.

### 2. Decompose into child tasks

Emit a JSON array of `{title, slug, spec_path?}` task objects (one per phase or vertical slice) and pipe it to:

```bash
maestro plan decompose <pln-id> --file -
```

The CLI writes each child task with `plan_id` set and transitions the plan `specified → planned`. The batch is validated atomically: no duplicate slugs within the batch, no collisions with existing tasks. A single validation error rejects the whole batch.

Example batch:

```json
[
  { "title": "Scaffold feature X",                  "slug": "scaffold-feature-x" },
  { "title": "Implement core use-case",             "slug": "impl-feature-x-core" },
  { "title": "Add unit + integration tests",        "slug": "test-feature-x" }
]
```

### 3. Inspect the plan

```bash
maestro plan show <pln-id>
maestro plan show <pln-id> --json
```

Prints the plan state, the spec path, and every child task with its current state.

### 4. Start work

The plan auto-advances `planned → in-progress` on the first `maestro claim <tsk-id>` (ADR-0011). It auto-advances to `completed` when every child reaches `shipped` or `abandoned`. There is no manual `plan start` or `plan complete` verb.

After the first child claim, load `maestro-task` and execute from there.

---

## Persist a human-readable plan note (optional)

The plan record in `.maestro/plans/plans.v2.jsonl` carries `title`, `slug`, `spec_path`, and `state` — it does not carry narrative. For non-trivial plans, persist a markdown sidecar so future sessions and reviewers have the rationale:

`.maestro/plans/<slug>.md`:

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

Collision handling: if `.maestro/plans/<slug>.md` already exists, append a numeric suffix (`<slug>-2.md`).

Skip this section entirely when no maestro project is detected.

---

## What this skill does not do

- Does **not** propose a contract. v2 contracts are derived from the spec's path globs and the task's diff; the agent does not author them in the plan.
- Does **not** lock a risk class. Maestro derives it from the diff; you cannot lower it from the plan.
- Does **not** open PRs. Each child task ends with `maestro task ship`; the PR opens through normal git flow before `ship`.
- Does **not** schedule iteration. Maestro stays passive (no cron, no daemon).

---

## Hand off to `maestro-task`

When tasks are created and the user approves, invoke the `Skill` tool with `skill: "maestro-task"`. Execution continues there. The handoff does not need a separate handoff envelope — `plan decompose` already emitted one, and the first `task claim` will emit `task:claim`.

---

## See also

- `maestro-design` — grill-protocol spec authoring (run before this skill).
- `maestro-task` — single-task execution loop (run after this skill for each child).
- `maestro-verify` — canonical verification protocol.
- `docs/cli-reference.md` — verb-by-verb reference.
- `docs/adr/0003-task-lifecycle.md`, `docs/adr/0011-exec-plan-auto-complete.md` — the lifecycle decisions this skill rides on.
