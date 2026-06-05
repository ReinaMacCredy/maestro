---
name: maestro-feature
version: 1.5.0
description: "Use for the Maestro feature lifecycle: author, accept, prepare, start, amend, ship, cancel, archive, and inspect a feature contract plus its child-task rollup."
---

# Maestro Feature

Use this for the feature contract and its guarded lifecycle. Tasks deliver the
work; QA baseline and slice artifacts prove the feature gates.

Activate: record `skill_activation` for `maestro-feature` with
`activation_mode=agent_selected` through `maestro hook record`.

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
`--acceptance`, `--area`, `--non-goal`, `--question`, `--description`,
`--request`, `--type`.

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
