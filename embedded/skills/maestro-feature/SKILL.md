---
name: maestro-feature
version: 1.4.0
description: Feature lifecycle layer for Maestro — the guarded five-state machine (proposed -> ready -> in_progress -> shipped/cancelled), its accept and ship gates, append-only amend, and feature/child-task archival.
---

# Maestro Feature

The how-to for the Maestro feature loop — the product contract a cluster of tasks delivers
against. A task links to its feature with `maestro task create --feature <id>`; this skill
owns the feature's own lifecycle. For the task loop itself see the `maestro-task` skill; for
the QA artifacts the gates check, see the `qa-baseline` and `qa-slice` skills.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-feature`, and `activation_mode` set to `agent_selected`.

## When to use

Use this skill whenever you propose, author, accept, prepare, start, ship, cancel, or archive
a feature, or need to read a feature's contract and task rollup (`show` / `list`).

## When NOT to use

- Do not hand-edit `.maestro/features/<id>/feature.yaml`, `baseline.md`, `qa-slices.yaml`, or
  `amend-log.yaml`. The verbs enforce the transition guards and append the audit trail; editing
  the files bypasses both. `notes.md` is the exception - it has no managing verb and is yours to
  append to freely (see "The design notes" below).
- Do not use `set` to change an accepted contract - `set` is proposed-only and errors once
  the contract is frozen. Grow a frozen contract with `amend` (append-only, audited).
- Do not skip states. `start` on a proposed feature errors and tells you to `accept` first;
  `ship` on a ready feature tells you to `start` first.
- Do not `cancel` a feature you mean to pause. `cancel` is terminal and abandons every live
  child task.

## Lifecycle and the guards on each step

State flow: `proposed -> ready -> in_progress -> shipped` (or `-> cancelled` from any
non-terminal state). `shipped` and `cancelled` are terminal.

    maestro feature new "<title>"                          # -> proposed; prints "created feature <id> (proposed)"
    maestro feature set <id> --acceptance "<criterion>" --area "<surface>"   # author the contract (proposed only; replace-per-field)
    maestro feature accept <id>                            # proposed -> ready; the accept gate (below); FREEZES the contract + baseline
    maestro feature prepare <id> --draft                   # stage an explicit child-task plan
    maestro feature prepare <id> --from <plan-file>        # create/explore/accept child tasks from the reviewed plan
    maestro feature start <id>                             # ready -> in_progress (low-level; prepare auto-starts when work is claimable)
    maestro feature amend <id> --add-acceptance "<…>" --reason "<why>"       # grow a frozen contract (ready/in_progress; append-only)
    maestro feature ship <id> --outcome "<one line>"   # in_progress -> shipped; the ship gate (below);
                                                        # --outcome records the result shown in `feature list --all`
    maestro feature cancel <id> --reason "<why>"           # any non-terminal -> cancelled; abandons live child tasks
    maestro feature show <id>                              # status, full contract, task counts
    maestro feature list [--all]                           # active features; --all adds terminal + archived

`set` is replace-per-field and repeatable: `--acceptance --area --non-goal --question
--description --request --type`; each repeated flag replaces that whole list. Both `accept`
and `ship` take `--dry-run` to preview the gate at exit 0 without transitioning.

## Prepare the implementation queue

After `feature accept`, use `feature prepare`, not ad hoc task choreography.

1. Draft or review the plan:

       maestro feature prepare <id> --draft
       maestro feature prepare <id> --from .maestro/features/<id>/prepare-draft.md

2. The plan is explicit and line-oriented:

       ## Task T1: Scaffold project
       check: package manifest exists and tests run
       blocker: dependency approval required for aws-cdk-lib

       ## Task T2: Implement API handlers
       after: T1
       check: GET /articles satisfies the API contract

3. `prepare --from` creates the full visible queue, moves each task through
   create -> explore -> accept, and starts the feature only when at least one
   task is ready and unblocked.
4. `blocker:` fields are the only approval blockers it creates. Do not rely on
   vague task text; write dependency, secrets, cloud, schema, paid service, or
   destructive-operation approval blockers explicitly.
5. `after:` dependencies create generated task blockers. Downstream tasks are
   visible but not claimable until prerequisite tasks verify.

## The design notes (`notes.md`)

`maestro feature new` scaffolds `.maestro/features/<id>/notes.md`. Unlike the verb-managed files
above, this one is yours to write: append your design reasoning as you go (off-spec decisions,
deviations, tradeoffs, gotchas) as one `YYYY-MM-DD  <note>` line at the moment you decide. It is
free-form prose, read by no gate; `maestro feature show` renders it, and `feature archive` carries
it with the feature. It is the running record behind the feature's contract and its Decision
records. To drive a design session that fills it, see the maestro-design skill.

## Feature fan-out (parallel child tasks)

Use when an in_progress feature has 2+ ready tasks that are independent -
no blocker edges between them and no overlapping files in their checks.

1. Confirm independence first: `maestro task list --ready`, then read each
   task's acceptance.yaml. Tasks that write the same files are NOT
   independent - serialize them, or isolate each agent in its own worktree.
2. Spawn one sub-agent per task, fresh context each. Each sub-agent owns its
   task end to end: `task claim <id>` -> work -> `task update --claim` as it
   goes -> `task complete --summary --claim --proof`. Sub-agents edit; they
   never stage, commit, or touch another task's files.
3. The conductor stays out of the edits: collect completions, run
   `maestro task verify <id>` per task (adversarial fan-out from the
   maestro-verify skill for high-risk ones), and commit per verified task.
4. The ship gate is the synthesis barrier: `maestro feature ship` refuses
   while any child is live, the baseline is stale, or slices are missing -
   run qa-slice, then `ship --outcome "<one line>"`.

Tasks that could touch the same files: isolate each agent in its own git
worktree (mechanics per agent: the harness orchestration menu). A sub-agent
freed up early can be redirected to the next ready task mid-flight.

## The accept gate (freezes the contract)

`maestro feature accept` refuses with `cannot accept <id> — contract incomplete:` and an
itemized fix list unless ALL of these hold:

1. The contract has at least one acceptance criterion (`feature set <id> --acceptance "…"`).
2. The contract has at least one affected area (`feature set <id> --area "…"`).
3. A behavior baseline exists at `.maestro/features/<id>/baseline.md` - run the `qa-baseline`
   skill to capture behavior before edits start.

On pass the feature moves to `ready` and the contract + baseline freeze: further growth is
`amend`-only. Open questions are carried, non-blocking.

## The ship gate (proves behavior)

`maestro feature ship` refuses with `cannot ship <id>:` and an itemized fix list unless ALL
of these hold:

1. No live child task (`draft/exploring/ready/in_progress/needs_verification`) - verify or
   abandon them, then re-ship. `verified` and terminal children do not block.
2. The baseline is fresh - no behavioral amend (an added acceptance or area) since the
   baseline's recorded `amend_log_position`. If stale, re-run `qa-slice`, extend the Scenario
   Matrix, and bump the position.
3. Every behavioral `[bl-NNN]` scenario in the baseline is covered by a counting QA slice
   (scenarios + evidence) in `qa-slices.yaml` - run the `qa-slice` skill. A baseline that
   records no behavioral surface needs no slice.

## Amend (grow a frozen contract)

    maestro feature amend <id> --add-acceptance "<criterion>" --reason "<why>"
    add-flags: --add-acceptance --add-area --add-non-goal --add-question   (--reason required)

`amend` is append-only and audited (it appends to `amend-log.yaml`); it works in `ready` and
`in_progress`, never on a terminal feature. Adding an acceptance criterion or an affected
area is a BEHAVIORAL amend - it re-opens the ship gate until the baseline and slices catch up
(ship conditions 2/3). Adding a non-goal or an open question does not.

## Archive (move terminal features out of the live scan)

`list` hides terminal (shipped/cancelled) features by default; `--all` shows them plus the
archive. Archiving moves a terminal feature out of the live scan entirely, into
`.maestro/archive/features/<id>/`:

    maestro feature archive <id>                  # terminal only; cascades the feature's terminal child tasks
    maestro feature archive --shipped             # archive every shipped feature (mutually exclusive with <id>)
    maestro feature archive <id> --dry-run        # preview the feature + child-task moves, write nothing
    maestro feature unarchive <id>                # restore the feature dir + its archived children

Archive cascades **children first, feature last**, so a re-run safely sweeps anything left.
A child a LIVE task still references (an open blocker) is skipped with a warning, not moved;
clear the reference and re-run to sweep it. Archive is idempotent - re-archiving an
already-archived feature is a no-op at exit 0. `show` and `list --all` read across the
archive boundary, so a historical reference still resolves.

## Defaults

Prefer the CLI verbs for every durable change - they keep the contract, the audit log, and
the QA artifacts intact. Read the frozen contract with `feature show` before you act; it is
fixed once `accept` runs and grows only by `amend`.

## Hand-off

maestro-design -> [maestro-feature] -> maestro-task -> maestro-verify -> [feature ship]

Next: just accepted -> the `maestro-task` skill (`task create --feature <id>`);
children all verified -> the `qa-slice` skill, then `ship --outcome "<one line>"`;
just shipped -> `feature archive`.
Related: `maestro-design` (fills notes.md and decides the contract), `maestro-verify`
(proves the child tasks), `qa-baseline` / `qa-slice` (the artifacts the accept/ship
gates check).
