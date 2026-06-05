---
name: maestro-task
version: 1.9.0
description: "Use for the Maestro task loop: create, explore, accept, claim, update, complete, block, verify, terminal task verbs, and harness self-improvement tasks."
---

# Maestro Task

Use this for task work. A task is the proof-gated unit of implementation; a
feature is the product contract it may deliver against.

Activate: record `skill_activation` for `maestro-task` with
`activation_mode=agent_selected` through `maestro hook record`.

## Use

- Create or prepare work: `create`, `explore`, `accept`.
- Pick up work: `claim --next` or `claim <id>`.
- Record progress: `update --summary` and/or `--claim`.
- Finish work: `complete --summary --claim --proof`, then verify.
- Handle pauses or terminal outcomes: `block`, `unblock`, `reject`,
  `abandon`, `supersede`.
- Act on harness improvement proposals surfaced by `status`, `task next`, or
  `harness list`.

## Do

```sh
maestro task create "<title>" [--feature F --lane L --risk R --check "<observable result>"]
maestro task explore <id>
maestro task accept <id>                 # locks acceptance, except tiny lane may skip
maestro task claim --next                # prints feature and dependency context
maestro task update <id> --summary "<note>" --claim "<evidence claim>"
maestro task complete <id> --summary "<what changed>" --claim "<claim>" --proof "<observed evidence>"
maestro task verify <id>
```

`verify` and `show` can omit `<id>` when `MAESTRO_CURRENT_TASK` is set.

## Evidence Gate

`complete --proof` records proof text and auto-runs verification. Verification
passes only when:

- task state is `needs_verification`
- at least one non-empty completion claim exists
- at least one proof source exists
- every claim text matches some proof or event text after whitespace
  normalization
- every configured verify command exits 0

Reliable closeout:

```sh
maestro task complete <id> \
  --summary "<what changed>" \
  --claim "cargo test: 40 passed, 0 failed" \
  --proof "cargo test: 40 passed, 0 failed"
```

Use concrete observed claims. A vague claim fails even when the work is real.
Use `maestro event create --task-id <id> --claim "<claim>"` only to repair or
add manual evidence after the default proof path is insufficient.

## Blockers And Terminal Verbs

```sh
maestro task block <id> --reason "<why>" [--by <task-NN|decision-NN|external>]
maestro task unblock <id> --blocker blk-NN
maestro task reject <id> --reason "<why>"
maestro task abandon <id> --reason "<why>"
maestro task supersede <id> --by <ref> --reason "<why>"
maestro task doctor
maestro task watch [<id>] [--interval N]
```

Open blockers stop both `claim` and `complete`. `reject`, `abandon`, and
`supersede` are terminal and cannot be undone.

## Triage And Loops

For unstructured audit/review/user-feedback backlogs:

1. Use read-only classifiers for raw untrusted items. They return severity,
   area, duplicate-or-new, and fixable-or-escalate only.
2. The conductor dedupes against `task list --all` and `feature list --all`.
3. Create or block real work through task verbs. The agent that read untrusted
   content does not run privileged actions.

For unknown-size work:

- Stop on a query, not a feeling: no ready tasks, or K discovery sweeps with
  zero new findings.
- Turn each new finding into a task immediately so it survives context loss.
- Claim, work, complete, verify, then re-check the stop condition.

## Harness Improvement

When Maestro surfaces recurring friction, act before unrelated work unless the
proposal is noise.

```sh
maestro harness list [--all]
maestro harness show <id>
maestro harness apply <id>                 # spawns an accepted standalone task
maestro harness measure <id>               # requires linked task verified
maestro harness dismiss <id> --reason "<why>"
```

If measurement still finds friction, the proposal reopens; if a measured
proposal regresses, it reopens.

## Stop

- Do not hand-edit `.maestro/tasks/<id>/task.yaml` or state history.
- Do not skip states. `claim` expects a ready task; verification owns
  `verified`.
- Do not use terminal verbs for "blocked, resume later"; use `block`.
- Do not complete with empty or unprovable `--claim`.
- If Git metadata is unavailable, do a targeted non-Git closeout review and say
  so in the proof.

## Hand-off

Pipeline: `maestro-design -> qa-baseline -> maestro-feature -> [maestro-task] -> maestro-verify -> qa-slice -> feature ship`

Next: task completed or proof failed -> `maestro-verify`; feature children all
verified -> `qa-slice`.
