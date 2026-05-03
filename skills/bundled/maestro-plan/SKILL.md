---
name: maestro-plan
description: Turn an approved brainstorm design into a concrete implementation plan by grounding on the real codebase or artifacts, consuming the accepted approach, constraints, assumptions, and unresolved decisions from the brainstorming phase, defining the done state, identifying risks and dependencies, sequencing phases and tasks, attaching validation to each step, and specifying the next executable move. In maestro projects, persists the plan to `.maestro/plans/<slug>.md` and emits task-batch JSON for `maestro-task`. Use when the user wants the phase after brainstorming, a phased approach, task breakdown, rollout sequence, implementation roadmap, or a concrete path from approved design to execution.
---

# Maestro Plan

Use this skill to convert an approved brainstorm design into an actionable plan.

## Enter plan mode

- Call the `EnterPlanMode` tool as the first action of this skill, before reading context or drafting the plan.
- Plan mode enforces read-only behavior so the plan is built from verified context without side effects.
- Stay in plan mode for the entire drafting pass: exploration, synthesis, phasing, and validation design.
- Do not edit files, run destructive commands, or start implementation while in plan mode. Use read-only tools (Read, Grep, Glob, Bash for inspection, etc.) only.
- When the plan is complete and ready to present, call `ExitPlanMode` with the full plan content as the argument. This both exits plan mode and presents the plan to the user for approval.
- If `EnterPlanMode` is unavailable (for example, already in plan mode or the tool is not exposed in this session), proceed with the skill anyway but keep the same read-only discipline and use `ExitPlanMode` at the end.
- If the user rejects the plan at `ExitPlanMode`, re-enter plan mode and revise rather than starting implementation.

## Start from the brainstorm handoff

- Treat this skill as the next phase after `maestro-brainstorm`.
- Read the brainstorm output first when it exists.
- Carry forward:
  - the approved approach or design
  - the relevant context discovered during brainstorming
  - constraints and assumptions
  - open questions
  - validation expectations
  - the final state from brainstorming
- If brainstorming ended in `needs-clarification` or `design-in-progress`, do not build a full plan yet.
- If the design has not been approved, ask for approval or continue design work instead of planning.
- If no brainstorm artifact exists, reconstruct the same handoff inputs from the user's request and available context.

## Ground on real context first

- Inspect the actual artifacts when the user references a repo, files, specs, tickets, docs, or prior decisions.
- Read enough of the current system to plan against reality, not assumptions.
- Base specific planning claims on verified context when it is available.
- Mark assumptions clearly when information is missing.

## Define the target

- Restate the approved design in one sentence.
- Define what done looks like.
- Identify explicit constraints, acceptance criteria, and out-of-scope areas.
- Distinguish firm requirements from preferences.
- Convert unresolved decisions from brainstorming into either:
  - decisions that must be made before implementation, or
  - explicit assumptions the plan will proceed under

Do not build a plan until the target is clear enough to avoid planning the wrong work.

## Plan around the real shape of the work

Identify:

- the systems or surfaces that will likely change
- the riskiest unknowns
- the dependencies that gate later work
- what can run in parallel and what must stay sequential
- what needs migration, rollout, or compatibility handling
- what verification is needed to prove the change works

Avoid generic task lists detached from the actual system.

## Build a phased plan

- Organize the work into 2 to 7 phases when the task is non-trivial.
- Give each phase a clear outcome, not just an activity label.
- Sequence phases by dependency order.
- Make tasks outcome-named, concrete, and falsifiable.
- Keep tasks small enough to execute without re-planning mid-task.

For each phase, include:

- purpose
- tasks
- dependencies
- verification checkpoint

## Attach validation to the plan

- Define the smallest relevant check for each phase.
- Call out tests, lint, type checks, builds, manual verification, or rollout checks as needed.
- Prefer the cheapest check that can falsify the phase.
- If later implementation should be test-first, identify the test surfaces up front.

## Surface risks and cut lines

- Identify what could derail the plan.
- Call out the highest-risk assumption or dependency.
- Separate must-have work from optional polish.
- State what can be deferred without undermining the goal.

## Return an execution-ready answer

Return a compact response in this shape:

1. Approved design from brainstorming
2. Current context
3. Constraints, assumptions, and unresolved decisions
4. Phased plan
5. Dependency and ordering notes
6. Validation strategy
7. Risks and cut lines
8. First execution step
9. State: `needs-clarification` or `ready-for-implementation`

## Keep the plan useful

- Do not start implementing.
- Do not produce a generic checklist with no ordering logic.
- Do not hide dependencies or risks.
- Do not over-plan beyond the actual complexity of the work.
- Match the depth of the plan to the risk and scope of the task.

## Propose a contract

Your plan must include `proposed_contract` with `allowed_files`, `forbidden_paths`, `risk_class`, and `amendment_budget`. The human reviews the plan including the contract.

Example:

```yaml
proposed_contract:
  allowed_files:
    - "src/features/auth/**"
    - "tests/unit/features/auth/**"
  forbidden_paths:
    - ".github/workflows/**"
    - "bun.lock"
    - "package.json"
  risk_class: medium
  amendment_budget:
    max_amendments: 3
    max_paths_per_amendment: 5
    forbidden_amendment_paths:
      - ".github/workflows/**"
      - "bun.lock"
      - "package.json"
```

`risk_class` is one of `low`, `medium`, `high`, `critical`. Be honest — `critical` for changes to auth, payments, secrets, or deploy infrastructure.

`amendment_budget` caps how many times the agent may expand `allowed_files` after the plan is locked. The agent must call `maestro contract amend --task <id> --add-path <p> --reason <r>` for each genuine scope change; failures are recorded.

## Persist the plan

When `.maestro/plans/` exists in the cwd or an ancestor (the project uses maestro), write the final approved plan to `.maestro/plans/<slug>.md` before handing off. Approved plans are durable, searchable references future sessions can read.

- **Slug:** kebab-case derived from the plan subject. Short (2 to 4 words). Example: `auth-jwt-migration`, `skill-bundle`, `docs-update`.
- **Collision handling:** if `.maestro/plans/<slug>.md` already exists, append a numeric suffix (`<slug>-2.md`, `<slug>-3.md`) rather than overwrite.
- **Format:** follow the existing convention in that directory.

```markdown
# <Title>

## Objective
<1-2 sentence restatement of the approved design>

## Scope
**In:** <what is included>
**Out:** <what is explicitly excluded>

## Research Findings
<context gathered during planning, grounded in verified files or artifacts>

## Tasks
- [ ] Task 1
- [ ] Task 2
- [ ] Task 3

## Verification
<commands, tests, or manual checks that prove the plan succeeded>

## Notes
<risks, cut lines, assumptions, references>
```

Skip this section entirely when no maestro project is detected.

## Hand off cleanly

- If `.maestro/` is present in the cwd or an ancestor (maestro project), hand off to `maestro-task`. Emit the phased plan as task-batch JSON matching `maestro task plan --schema`: each phase becomes a task with `name`, `title`, `description`, and `blockedBy` wiring the phase dependencies. `maestro-task` runs `maestro task plan --file -` and claims the first task via `--start <name>`.
- If no maestro project is detected, switch to a generic implementation skill when the user wants execution.
- When cross-session transfer is needed at any point during implementation, hand off to `maestro-handoff`. `maestro-handoff` writes a rich brief and launches a new session via `maestro handoff --prompt-file`.
- Preserve the approved design unless new verified context forces the plan to reopen the design decision.
- If the implementation skill is TDD-based, make the likely test entry points explicit in the plan.
