---
name: maestro-handoff
description: Read handoff envelopes left by previous agents at the start of a session or when picking up a task in a maestro-initialized project. Use whenever you suspect a prior session left context behind — task was claimed by another agent, a block was raised, or a new shell is taking over an unfinished workflow.
---

# Maestro Handoff

Handoff is **passive** in maestro. Lifecycle verbs drop a small JSON envelope on disk; the next agent reads it.

## When to read this skill

- Starting a session in a `.maestro/` project and you want to know what the last agent left behind.
- Picking up a task another agent claimed or blocked.
- Debugging why a follow-up agent did not pick up context.

## The model

Emitting agent does nothing extra — the lifecycle verb writes the envelope.
Receiving agent reads `.maestro/handoffs/<hnd-...>.json` directly. The file on disk is the whole contract.

## Which verbs emit envelopes

| Verb | Emits | Envelope `trigger_verb` |
|---|---|---|
| `maestro task claim` | yes | `task:claim` |
| `maestro task block` | yes | `task:block` |
| `maestro task ship` | no (roadmap) | — |
| `maestro task verify` | no (roadmap) | — |
| `maestro task abandon` | no (roadmap) | — |

`HandoffTrigger` in the port reserves all five values; only claim and block are wired. Treat the other three as not-yet-handoff-emitting until that changes.

## Envelope schema

Path: `.maestro/handoffs/<hnd-<base36>-<rand>>.json`

```json
{
  "id": "hnd-...",
  "task_id": "tsk-...",
  "trigger_verb": "task:claim" | "task:block",
  "created_at": "<ISO-8601>",
  "agent_id":     "<optional>",
  "worktree_path":"<optional>",
  "spec_path":    "<optional>",
  "reason":       "<optional, present on task:block>"
}
```

Filename is the envelope id; the task id lives inside the file.

## How to find envelopes for a task

Scan recent envelopes and read each:

```bash
ls -1t .maestro/handoffs/*.json | head -10 | xargs -I{} jq '{id, task_id, trigger_verb, created_at, reason}' {}
```

Then filter by `task_id`. The `*.json` glob matches envelope files only — pickup sidecars live at `<id>.picked_up.json`.

## Pickup protocol

1. Read the envelope. Confirm `task_id` matches what you intend to pick up.
2. Check `trigger_verb`:
   - `task:claim` — a prior agent had it; verify they are gone before re-claiming.
   - `task:block` — task is blocked. Read `reason`; resolve before re-claim.
3. Re-claim: `maestro task claim <task_id> --agent <your-agent-id>`.
4. Continue the verification loop per `maestro-verify`.

## MCP tools (when available)

The MCP surface mirrors direct file reads. Use these instead of `ls`/`jq` when an MCP client is wired:

| MCP tool                  | Purpose                                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------ |
| `maestro_handoff_list`    | List open envelopes. Filters: `task_id`, `trigger_verb`, `include_picked_up` (default `false`).  |
| `maestro_handoff_show`    | Fetch one envelope by `hnd-*` id. Returns the envelope and pickup metadata when present.         |
| `maestro_handoff_emit`    | Write an envelope. Only needed when emitting outside the lifecycle verbs that already emit.      |
| `maestro_handoff_pickup`  | Mark an envelope picked up via a `<id>.picked_up.json` sidecar. Second pickup returns `HANDOFF_ALREADY_PICKED_UP`. |

`maestro_handoff_pickup` is a bookkeeping mark; it does **not** claim the task — call `maestro_task_claim` after pickup.

## Hand off cleanly

The next phase after this skill is `maestro-task` (after `task claim` re-establishes ownership) or `maestro-verify` (if the prior agent left mid-verification and the verdict loop should resume).

Pass a re-claimed task whose envelope context — `agent_id`, `worktree_path`, `spec_path`, `reason` — has been internalized. Not just an envelope you glanced at.
Do not invoke spec authoring or planning from this skill; this skill is read-only handoff plumbing.

## See also

- `maestro-task` — full task lifecycle.
- `maestro-verify` — canonical verification protocol after re-claim.
