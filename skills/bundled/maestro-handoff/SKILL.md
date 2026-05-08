---
name: maestro-handoff
description: Hand off current work to another session via `maestro handoff`. Covers transfer to a different agent (codex to claude or vice versa) AND transfer to another session of the same agent (claude to claude, codex to codex). Use when the user says "handoff", "hand off to codex/claude/new session", "drop a packet", "pickup handoff", "list open handoffs", or wants to pass work across any session boundary through maestro's native launcher.
user-invocable: true
---

# Maestro Handoff

You are handing off the current task to another session via maestro's native launcher. Your job is to write a rich handoff brief and launch the receiving session via `maestro handoff --prompt-file`.

**User's arguments:** $ARGUMENTS

---

## Prerequisites

1. `maestro` must be on PATH. If missing, tell the user to install maestro and stop.
2. For mission/task linkage, the current working tree (or an ancestor) must contain `.maestro/`. Handoffs work outside maestro projects too, but `refs` will be empty.
3. Before requesting a handoff for task work, run the verification protocol in `maestro-verify`. The receiving session inherits the task's contract and Evidence; hand off after `maestro task verify` is clean and `maestro verdict request` has been run so the receiver can see the current Verdict.

## What a maestro handoff is

A portable transfer artifact persisted on disk:

- `prompt.md`: the brief sent to the receiving session
- `handoff.json`: metadata (agent, model, status, refs, timing)
- `output.log`: stdout/stderr from the launched session

**Every packet lands in one global store: `~/.maestro/handoff/<id>/`.** There is no per-project store. `--task-id` links the packet to a task for continuation and ownership transfer on pickup, but it does not change where the packet is written. `promptPath` and `outputPath` in the JSON output are relative; resolve them against `~/` (the global store root). `maestro handoff list` scans the single global store, so handoffs created in one working directory are visible from any other.

Packets are detached by default. The launcher returns immediately with a handoff id. The launched receiver prompt now tells the new session to run `maestro handoff pickup --id <id> --json` before any other work so ownership and packet state stay aligned. Prompt-only packets can be picked up from any working directory. Task-linked packets must be picked up from their source project unless the operator explicitly passes `--standalone` to discard task linkage.

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

Mention of `tsk-abc123`, "for task X", "link to task Y": add `--task-id <id>`. Task-linked packets carry the task's continuation summary and transfer claim ownership on pickup when they are picked up from the source project.

If no `--task-id` is passed and the project has exactly one active continuation, maestro links to it automatically. With zero or multiple active continuations, the packet is standalone.

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

Do not wait unless the user explicitly asked. The receiver runs detached, but the launched session is expected to consume its own packet immediately on startup.

## Pickup flow

When the user says "pickup handoff", "take over handoff":

1. `maestro handoff list --open --json` to enumerate open packets (this list is global; packets from other projects may appear).
2. Single open packet: `maestro handoff pickup --json`.
3. User specified an id: `maestro handoff pickup --id <id> --json`.
4. Multiple packets and no id: the CLI errors with a clean list of open packets. Surface the list to the user and ask which.

If the session environment provides a detected agent (Claude Code / Codex) the CLI uses it; otherwise it defaults to the packet's own `agent` field. Pass `--agent codex|claude` and `--session <id>` together to override identity explicitly.

Pickup auto-claims a linked task only when the current working directory matches the packet's source project. From another project, Maestro errors with the source path and a concrete `cd ... && maestro handoff pickup --id <id> --json` command. Use `maestro handoff pickup --id <id> --standalone --json` only when you intentionally want the prompt without resuming the linked task. Prompt-only packets (no `refs.taskId`) create no task and can be picked up anywhere.

Pickup semantics including stale-claim transfer and contract inheritance live in `./reference/pickup.md`.

## MCP equivalents

When invoked from an MCP-connected agent, the read and pickup verbs are also exposed as tools on the maestro MCP server. Launching new packets is intentionally CLI-only.

- `maestro_handoff_list` — read-only, project-scoped, paginated. Optional filters: `openOnly` (boolean, mirrors `--open`), `displayState` (`open|consumed|completed|failed`, mutually exclusive with `openOnly`), `taskId` (filter to packets linked to a specific task), `agent` (`codex|claude|hermes`).
- `maestro_handoff_show` — read-only, project-scoped. Takes `{ id }`. Returns code `HANDOFF_NOT_FOUND` when the packet does not exist or belongs to another project.
- `maestro_handoff_open_for_task` — read-only, project-scoped. Takes `{ taskId }`. Returns ids of open packets linked to that task, newest first. Use this when resuming work on a known task to find a waiting packet without scanning the full list.
- `maestro_handoff_pickup` — takes `{ id, actorAgent, actorSessionId?, ownerId?, standalone? }`. `actorSessionId` defaults to the MCP session id (`MAESTRO_SESSION_ID`/`CLAUDECODE_SESSION_ID`/`CODEX_THREAD_ID`, else `username@host`); `ownerId` defaults to `buildTaskOwnerId(actorAgent, actorSessionId)`. Error codes: `HANDOFF_NOT_FOUND`, `ALREADY_CONSUMED`, `CROSS_PROJECT_PICKUP`, `HANDOFF_TASK_COMPLETED`, `HANDOFF_TASK_BLOCKED`, `OWNERSHIP_CONFLICT`.

## Reference

- `./reference/brief-template.md`: longer brief example with a realistic scenario
- `./reference/pickup.md`: pickup semantics, stale-claim transfer, contract inheritance
