---
name: maestro-task
version: 1.1.0
description: Feature and task workflow layer for operating the Maestro harness.
---

# Maestro Task

The full how-to for the Maestro task loop. `.maestro/harness/HARNESS.md` carries the
always-loaded cheat-sheet; this skill is the on-demand reference for the rarer verbs,
the evidence gate, and the gotchas.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-task`, and `activation_mode` set to `agent_selected`.

## When to use

Use this skill whenever you create, claim, advance, block, or verify Maestro tasks, or
need a verb you don't run every day (reject / abandon / supersede / doctor / watch).

## When NOT to use

- Do not hand-edit `.maestro/tasks/<id>/task.yaml` or its state. The verbs append durable
  state history and enforce the transition guards; editing the file bypasses both.
- Do not skip states. `claim` on a `draft` errors and tells you to `explore` first; you
  cannot hand-write the `verified` state (the verify subsystem owns that transition).
- Do not use `reject` / `abandon` for "blocked, will resume later" - that is `block`.
  Those three verbs are terminal.
- Do not `complete` with an empty or unbackable `--claim`. An empty claim records nothing
  and `verify` fails with "no completion claims found in task history".

## Lifecycle and the guards on each step

State flow: `draft -> exploring -> ready -> in_progress -> needs_verification -> verified`

    maestro task create "<title>" [--feature F --lane L --risk R]   # -> draft; prints "created <id>"
    maestro task explore <id>          # draft -> exploring
    maestro task accept <id>           # exploring -> ready; LOCKS acceptance (lane "tiny" may skip the lock)
    maestro task claim <id>            # ready -> in_progress; requires acceptance locked AND no open blockers
    maestro task update <id> --summary "<note>" --claim "<evidence>"   # records progress, no state change
    maestro task complete <id> --summary "<what>" --claim "<evidence>" # in_progress -> needs_verification; requires no open blockers
    maestro task verify <id>           # the gate (below); on pass needs_verification -> verified

`update` requires at least one of `--summary` / `--claim`. `verify` and `show` resolve
`<id>` from `MAESTRO_CURRENT_TASK` when you omit it.

## The evidence gate (the thing that bites)

`maestro task verify` does not just flip state. It writes a verification report and only
applies `verified` when ALL of these hold - otherwise it prints each failure and exits
non-zero, leaving the task in `needs_verification`:

1. State is `needs_verification`. Else: `task is <state>, expected needs_verification`.
2. At least one non-empty completion claim exists in the task's history (recorded by
   `complete --claim` / `update --claim`). Else: `no completion claims found in task history`.
3. At least one proof source exists. Else: `missing proof: no task events or proof artifacts found`.
4. EVERY claim string-matches some evidence claim (whitespace-normalized). Else:
   `claim not backed by events/proof: <claim>`.
5. EVERY configured verify command exits 0. Else: `verify command failed: <cmd> (exit N)`.

A claim is "backed" when its text matches one of these evidence sources for the task:

- A recorded hook event - its `claim` / `message` / `claims`, plus an auto-synthesized
  `<tool> <tool_input_hash>` for each successful tool call (hooks record these for you).
- A `claim:`-prefixed line in any text file under `.maestro/tasks/<id>/evidence/` or `proof/`.
- A `task_proof` event you record explicitly with `maestro event create`.

Reliable recipe - record the claim as evidence, then complete with the same text:

    maestro event create --task-id <id> --claim "cargo test: 40 passed, 0 failed"
    maestro task complete <id> --summary "<what changed>" --claim "cargo test: 40 passed, 0 failed"
    maestro task verify <id>     # the completion claim matches the recorded event -> passes

The claim strings must match (whitespace aside). Vague claims you cannot point evidence at
will fail step 4 even when the work is real.

## Blockers (overlay any state)

    maestro task block <id> --reason "<why>" [--by <target>]   # adds blk-NN; prints "blocked <id> (blk-NN)"
    maestro task unblock <id> --blocker blk-NN                  # resolve by the blocker's own blk- id

`--by` routes by prefix: `task-NN` -> task, `decision-NN` -> decision, anything else -> an
external ref; omit it for a human blocker. Open blockers stop both `claim` and `complete`.

## Terminal verbs and tools

    maestro task reject <id> --reason "<why>"               # -> rejected; work will not be done
    maestro task abandon <id> --reason "<why>"              # -> abandoned; giving up on this task
    maestro task supersede <id> --by <ref> --reason "<why>" # -> superseded; replaced by <ref>
    maestro task doctor                                     # check the blocker graph; non-zero exit on cycles/dangling refs
    maestro task watch [<id>] [--interval N]                # live snapshot loop

`reject` / `abandon` / `supersede` are legal from any non-terminal state - including
`verified`, so a finished task can still be rejected, abandoned, or superseded - and cannot
be undone. Once a task is itself rejected, abandoned, or superseded, no further transition
is allowed. Record `--by` as the id of the task that replaces this one.

## Defaults

Prefer the CLI verbs for every durable change - they keep state history and proof intact.
Read the locked `acceptance.yaml` before you act; those checks are fixed once `accept` runs.
