# Handoff Pickup Semantics

## Commands

```bash
maestro handoff list --open --json       # enumerate open packets
maestro handoff show <id> --json         # inspect a specific packet without consuming
maestro handoff pickup --json            # consume the only open packet (errors if multiple)
maestro handoff pickup --id <id> --json  # consume a specific packet
```

## Auto-detection

`pickup` detects the current agent and session from the environment only when
one of these env vars is set and matches a live agent process:

- `CLAUDECODE=1` plus a readable `~/.claude/sessions/<ppid>.json` (set by
  Claude Code at the top of its process tree).
- `CODEX_THREAD_ID` (set by Codex).

Anywhere else -- a plain shell, CI, a nested subprocess whose `ppid` no
longer points at the agent, a script invoked by a tool call -- auto-detection
returns nothing and `pickup` fails with `"No agent specified for handoff
pickup"`. In those cases, pass `--agent codex|claude` and `--session <id>`
explicitly. Assume you must pass them unless you can confirm your process
is the direct agent.

## Ambiguity

If multiple open packets exist and no `--id` is passed, `pickup` errors with a
clean list of candidate packets. Surface that list to the user and ask which
one to pick up. Do not guess.

## Task linkage

- **Task-linked packet** (packet has `refs.taskId`): pickup immediately takes
  over the linked task, switches task ownership to the current session, and
  follows any active task contract lock to the new owner.
- **Prompt-only packet** (no `refs.taskId`): pickup loads the prompt and
  marks the packet consumed. No task is created or claimed.

## Stale-claim transfer

When another session currently holds the linked task, pickup transfers the
claim silently and records a `handoff_claim_transferred` event in the task's
continuation history. This is the intended "agent B picks up work that agent
A started" path.

If the linked task was deleted out of band, pickup unlinks silently and
proceeds as a standalone pickup. No error.

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
