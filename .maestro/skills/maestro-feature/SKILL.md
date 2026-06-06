---
name: maestro-feature
version: 1.6.1
description: "Use for the Maestro feature lifecycle: author, accept, prepare, start, amend, ship, cancel, archive, and inspect a feature contract plus its child-task rollup."
---

# Maestro Feature

Use this for the feature contract and its guarded lifecycle. Tasks deliver the
work; QA baseline and slice artifacts prove the feature gates.

Activate:
`maestro hook record --event skill_activation --skill maestro-feature`

## Use

- Author or inspect a feature: `new`, `set`, `show`, `list`.
- Freeze a proposed contract: `accept`.
- Turn an accepted contract into tasks: `prepare`.
- Grow a frozen contract: `amend`.
- Finish or retire the feature: `ship`, `cancel`, `archive`, `unarchive`.

## Do

```sh
maestro feature new "<title>"                         # -> proposed
maestro feature set <id> --acceptance "<check>" --area "<surface>"
maestro feature accept <id>                           # -> ready, requires qa-baseline
maestro feature prepare <id> --draft                  # reviewable child-task plan
maestro feature prepare <id> --from <plan-file>       # create/explore/accept tasks
maestro feature ship <id> --outcome "<one line>"      # -> shipped, requires qa-slice
maestro feature archive <id>                          # terminal features only
```

`set` works only while `proposed` and replaces each repeated field:
`--acceptance`, `--area`, `--non-goal`, `--question`, `--clear-questions`,
`--description`, `--request`, `--type`.

## Spec/Plan Intake

When the user gives a SPEC, PLAN, issue, PRD, brainstorm note, or rough prose,
the agent converts it into Maestro artifacts. Maestro stores the result; it does
not infer product scope from arbitrary prose.

1. Read the source document completely and preserve explicit constraints.
2. Extract the feature contract:
   title, request, description, acceptance criteria, affected areas,
   non-goals, and unresolved questions.
3. Create or update the proposed feature with `feature new` and `feature set`.
4. Use `qa-baseline` to write the baseline, then run `feature accept`.
5. Extract implementation work into an explicit prepare plan file when the
   source is not already a clean plan.
6. Run `feature prepare <id> --from <plan-file>` and inspect created tasks.

Prepare plans may be agent-authored. Keep every task explicit and observable:

```markdown
## Task Plan

- Task T1: Add the API route
  - check: GET /articles returns compact records

2. T2: Add retry support
   - after: T1
   - check: POST /retry retries failed jobs
   - blocker: deployment approval required
```

`prepare --from` parses explicit task entries and `check:`, `after:`, and
`blocker:` fields. If the source only describes intent, write the concrete
plan first instead of expecting the CLI to infer tasks.

`prepare --from` expects a visible plan:

```markdown
## Task T1: Scaffold project
check: package manifest exists and tests run
blocker: dependency approval required for aws-cdk-lib

## Task T2: Implement API handlers
after: T1
check: GET /articles satisfies the API contract
```

`blocker:` creates an approval blocker. `after:` creates a task dependency.
Prepare starts the feature only when at least one child task is accepted and
unblocked.

## Gates

Accept passes only when the feature has:

- at least one acceptance criterion
- at least one affected area
- `.maestro/features/<id>/baseline.md` from `qa-baseline`

On pass, the contract and baseline freeze. Later growth uses:

```sh
maestro feature amend <id> --add-acceptance "<check>" --reason "<why>"
```

Behavioral amends, meaning added acceptance or area, make the ship gate require
fresh baseline/slice coverage.

Ship passes only when:

- no live child tasks remain
- the baseline is fresh for behavioral amends
- every behavioral `[bl-NNN]` in the baseline has a counting slice in
  `qa-slices.yaml`

Use `accept --dry-run` or `ship --dry-run` to preview a gate without changing
state.

## Fan-out

Use feature fan-out only when 2+ ready tasks are independent.

1. Confirm with `maestro task list --ready` and each task's `acceptance.yaml`.
   Same files or dependency edges mean serialize, or isolate in separate
   worktrees.
2. Spawn one fresh sub-agent per task. Each owns:
   `task claim <id> -> work -> task complete --summary --claim --proof`.
3. The conductor collects completions, runs `maestro task verify <id>`, commits
   verified task slices, then runs `qa-slice` before ship.

## Stop

- Do not hand-edit `feature.yaml`, `baseline.md`, `qa-slices.yaml`, or
  `amend-log.yaml`. Use verbs so guards and audit trails stay intact.
- Do not use `set` after accept; use `amend`.
- Do not cancel a feature you only mean to pause. `cancel` is terminal and
  abandons live child tasks.
- Do not ship around QA blockers. Fix the task, baseline, or slice evidence.

## Hand-off

Pipeline: `maestro-design -> qa-baseline -> [maestro-feature] -> maestro-task -> maestro-verify -> qa-slice -> [feature ship]`

Next: accepted feature -> `maestro-task`; all children verified -> `qa-slice`,
then `feature ship --outcome "<one line>"`; shipped -> archive if you mean to
retire it.
