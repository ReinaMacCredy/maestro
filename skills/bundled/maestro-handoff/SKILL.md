---
name: maestro-handoff
description: Hand off current work to another session via `maestro handoff`. Covers transfer to a different agent (codex to claude or vice versa) AND transfer to another session of the same agent (claude to claude, codex to codex). Use when the user says "handoff", "hand off to codex/claude/new session", "drop a packet", "pickup handoff", "list open handoffs", or wants to pass work across any session boundary through maestro's native launcher.
user-invocable: true
---

# Maestro Handoff

You are handing off the current task to another session via maestro's native launcher. Your job is to write a rich handoff brief and launch the receiving session via `maestro handoff --prompt-file`.

**User's arguments:** $ARGUMENTS

> Requires `maestro >= 0.57.0`. `--prompt-file` is the key integration point. On older versions the flag is rejected and the CLI falls back to auto-generating a brief from the one-line task description.

---

## Prerequisites

1. `maestro` must be on PATH. If missing, tell the user to install maestro and stop.
2. For mission/task linkage, the current working tree (or an ancestor) must contain `.maestro/`. Handoffs work outside maestro projects too, but `refs` will be empty.

## What a maestro handoff is

A portable transfer artifact persisted at `.maestro/launches/<id>/`:
- `prompt.md`: the brief sent to the receiving session
- `launch.json`: metadata (agent, model, status, refs, timing)
- `output.log`: stdout/stderr from the launched session

Packets are detached by default. The launcher returns immediately with a handoff id; the receiver runs in the background and can be picked up later by a different session.

## Parsing arguments

Parse `$ARGUMENTS` to determine:
1. Target agent and model
2. Worktree (isolated git copy)?
3. Task link?
4. Task description (becomes the packet title plus the core of the brief)

### Agent and model

| User says | `--agent` | Default model |
|---|---|---|
| (nothing), "codex" | `codex` | `gpt-5.4` |
| "claude", "opus" | `claude` | `opus` |
| "sonnet" | `claude` | `sonnet` |
| "haiku" | `claude` | `haiku` |
| "new session", "fresh session", "another claude/codex" | match current agent | current default |

If the user names a specific model ("codex gpt-5.4-fast", "claude sonnet 4.7"), pass `--model <exact>`.

### Worktree

"in a worktree", "worktree", "isolated": add `--worktree <slug> --base $(git branch --show-current)`. Derive a short slug from the task description.

### Task link

Mention of `tsk-abc123`, "for task X", "link to task Y": add `--task-id <id>`. Task-linked packets carry the task's continuation summary and transfer claim ownership on pickup.

## Writing the brief

The receiving session starts with zero context. Write a self-contained brief to `/tmp/maestro-handoff-<timestamp>.md`:

```
## Task
[Imperative description of what to do]

## Context
[Why this exists, background the receiver needs]

## Relevant Files
- `path/to/file.ts`: what it does and why it matters
- `path/to/other.ts`: what it does and why it matters

## Current State
[What is done, what works, what does not]

## What Was Tried
- Approach 1: why it failed or was abandoned
- Approach 2: partial success, with the remaining gap

## Decisions
- Decision 1 plus rationale
- Decision 2 plus rationale

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2

## Constraints
- Do not do X
- Must preserve Y
```

Preserve the user's task qualifiers:
- "investigate without editing" becomes an explicit "DO NOT edit any files".
- "refactor" does not become "rewrite".
- "fix" means implement the fix.

A longer brief template lives in `./reference/brief-template.md`.

## Launching

Default:
```bash
maestro handoff \
  --agent codex \
  --prompt-file /tmp/maestro-handoff-<ts>.md \
  --name "<short title>" \
  --json
```

With worktree:
```bash
base=$(git branch --show-current)
maestro handoff \
  --agent codex \
  --worktree <slug> --base "$base" \
  --prompt-file /tmp/maestro-handoff-<ts>.md \
  --name "<short title>" \
  --json
```

With task link:
```bash
maestro handoff \
  --agent claude \
  --task-id tsk-abc123 \
  --prompt-file /tmp/maestro-handoff-<ts>.md \
  --name "<short title>" \
  --json
```

## After launch

Parse the JSON response. Report to the user:

```
Handed off to <agent> (<model>). Handoff id: <id>
Follow: maestro handoff show <id>
Pickup later: maestro handoff pickup --id <id>
```

Do not wait unless the user explicitly asked. The receiver runs detached.

## Pickup flow

When the user says "pickup handoff", "take over handoff":

1. `maestro handoff list --open --json` to enumerate open packets.
2. Single open packet: `maestro handoff pickup --json`.
3. User specified an id: `maestro handoff pickup --id <id> --json`.
4. Multiple packets and no id: the CLI errors with a clean list of open packets. Surface the list to the user and ask which.

Pickup auto-claims any linked task. Prompt-only packets (no `refs.taskId`) create no task.

Pickup semantics including stale-claim transfer and contract inheritance live in `./reference/pickup.md`.

## Reference

- `./reference/brief-template.md`: longer brief example with a realistic scenario
- `./reference/pickup.md`: pickup semantics, stale-claim transfer, contract inheritance
