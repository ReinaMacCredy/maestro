# Handoff Pickup Semantics

## Commands

```bash
maestro handoff list --open --json       # enumerate open packets
maestro handoff show <id> --json         # inspect a specific packet without consuming
maestro handoff pickup --json            # consume the only open packet (errors if multiple)
maestro handoff pickup --id <id> --json  # consume a specific packet
maestro handoff pickup --id <id> --standalone --json  # consume without resuming a linked task
```

## Identity resolution

`pickup` resolves identity in this order:

1. If `--agent` and `--session` are both passed, use them. (Either both or
   neither -- passing one without the other errors.)
2. Otherwise, if the environment exposes a detected agent session
   (`CLAUDECODE=1` + a matching `~/.claude/sessions/<ppid>.json`, or
   `CODEX_THREAD_ID`), use it.
3. Otherwise, fall back to the packet's own `agent` field. For task-linked
   packets, a synthesized session id derived from the calling shell's `ppid`
   is used for ownership tracking.

The bare `maestro handoff pickup --id <id>` form works from any shell for
prompt-only packets. For task-linked packets, Maestro checks that the current
working directory matches the packet's source project before it resumes the
linked task. Pass explicit flags only when you need to override identity
(e.g., running recovery from an operator account).

Launched handoff receivers are instructed to run `pickup` as their first
step. That keeps the packet state aligned with the task state instead of
leaving a detached packet falsely open after the work is already done.

## Ambiguity

If multiple open packets exist and no `--id` is passed, `pickup` errors with a
clean list of candidate packets. Surface that list to the user and ask which
one to pick up. Do not guess.

`maestro handoff list --open` only shows packets still in the launching or
launched state. Completed, failed, or consumed packets are not considered open,
even if they were never explicitly consumed.

## Task linkage

- **Task-linked packet** (packet has `refs.taskId`): pickup immediately takes
  over the linked task only when pickup runs from the packet's source project.
  In that case it switches task ownership to the current session and follows
  any active task contract lock to the new owner. From a different project,
  pickup errors with the source path and the exact `cd ... && maestro handoff
  pickup --id <id> --json` command needed to preserve linkage.
- **Prompt-only packet** (no `refs.taskId`): pickup loads the prompt and
  marks the packet consumed. No task is created or claimed.
- **Standalone override** (`--standalone`): pickup consumes the packet from
  any working directory without resuming the linked task. This is the escape
  hatch when you intentionally want only the prompt and accept that task
  ownership, continuation state, and contract state stay with the source
  project.

## Stale-claim transfer

When another session currently holds the linked task, pickup transfers the
claim silently and records a `handoff_claim_transferred` event in the task's
continuation history. This is the intended "agent B picks up work that agent
A started" path.

If the linked task was deleted out of band in the source project, pickup
unlinks silently and proceeds as a standalone pickup. No error.

## Contract inheritance

Active task contracts follow the new owner by default. The policy key
`contracts.staleReclaimContractPolicy` in `.maestro/config.yaml` can be set
to `block` to refuse stale-claim transfer when a contract is active; the
default is `allow`.

## What pickup does not do

- Does not re-read `prompt.md` from disk. The launch command array already
  carries the prompt.
- Does not resurrect a dead launched process. If the original agent crashed,
  pickup creates a fresh session on the current machine.
- Does not merge packets. One pickup consumes exactly one packet.
