---
name: maestro-task
description: Use at the start of any multi-step work in any project that uses maestro, and throughout task execution. Decompose plans into `maestro task` entries before starting (including converting /maestro-plan or /planner output into a task batch), claim one task at a time, keep continuation state fresh, and close with a receipt. Agent auto-invokes without user action whenever a `.maestro/` directory is present in the current working tree or an ancestor.
---

# Maestro Task

You are working in a maestro-initialized project. Decompose multi-step work into maestro tasks before starting. Keep continuation state fresh while working. Close with a receipt.

---

## When to activate

Auto-activate when:
1. The user asks for a multi-step implementation.
2. You just received output from `/maestro-plan`, `/planner`, or a markdown checklist. **Convert it to a task batch before executing.**
3. The user names a task id (`tsk-abc123`) or says "resume this task" / "work on X".
4. Starting a fresh session in a maestro project (`.maestro/` exists in cwd or ancestor).

Do not activate for one-liner edits or read-only questions.

## Hard rules

1. **One task `in_progress` per session** unless the user explicitly passes `--force`.
2. **Every completion carries `--reason`.** The receipt is shared context for future sessions.
3. **Update continuation state on meaningful change only.** Current state shifted, next action changed, a decision was made, blockers appeared. Not on every trivial edit.
4. **Blockers block transitions.** A task cannot move to `in_progress` or `completed` with unresolved blockers.
5. **Handoff is for cross-session transfer only.** Same-session resume uses continuation state, not handoffs.

## Session detection

Every ownership verb (`task next`, `task claim`, `task create --status in_progress`, `task update --status in_progress`, `task unclaim`, `task heartbeat`, `task mine`, `task plan --start <name>`, any `task contract *` that mutates) needs to know which session is acting. Maestro auto-detects the session from two env vars:

- `CLAUDECODE=1` plus a valid `~/.claude/sessions/<ppid>.json` (set by Claude Code).
- `CODEX_THREAD_ID` (set by Codex).

When the skill is loaded inside a running Claude Code or Codex session, auto-detection works — no extra flags needed. In any other context (a plain shell, CI, a nested subprocess whose `ppid` no longer points at the agent process), auto-detection fails and the command errors with: `"Could not detect current session for task ownership"`.

Fix: pass `--session <id>` explicitly to every ownership verb. Use any stable identifier (e.g., `--session claude-nested-<timestamp>`). Read-only verbs do not take `--session` and will reject it: `task ready`, `task stuck`, `task similar`, `task show`, `task plan --file` (without `--start`).

## Converting a plan into a task batch

When you have a plan (from `/maestro-plan`, `/planner`, or a markdown checklist):

1. Read the plan. Identify phases or steps as candidate tasks.
2. Map each step to a task JSON object. Use `description` for detail, `type` for kind (`task|bug|feature|epic|chore`), `priority` (0-4, 0 highest) for ordering hints, `blockedBy` for sequencing.
3. Submit atomically via `maestro task plan --file -`:

```bash
cat <<'JSON' | maestro task plan --file - --start scaffold
{
  "batchId": "plan-<short-slug>",
  "tasks": [
    {
      "name": "scaffold",
      "title": "Scaffold feature X",
      "description": "Create the feature directory, wire index.ts, add skeleton services.ts.",
      "type": "feature",
      "priority": 1
    },
    {
      "name": "impl",
      "title": "Implement the core use-case",
      "description": "Build the use-case behind the port; cover happy path first.",
      "blockedBy": ["scaffold"]
    },
    {
      "name": "tests",
      "title": "Add unit + integration tests",
      "blockedBy": ["impl"]
    },
    {
      "name": "ship",
      "title": "Open PR",
      "type": "chore",
      "blockedBy": ["tests"]
    }
  ]
}
JSON
```

`--start <name>` claims the named task and flips it to `in_progress` in the same command. `batchId` makes retries idempotent (receipt persists under `.maestro/tasks/batches/`). Any validation error rejects the whole batch.

Report to the user: tasks created, which one is `in_progress`, the task id of each.

**Why plan-first:** atomic create, referential integrity (`blockedBy` / `parent` resolved in one pass), idempotent retry, no reactive one-at-a-time drift.

Full schema: `maestro task plan --schema`. Longer examples: `./reference/plan-conversion.md`.

## Single-task shortcut

For a one-task job (no plan):
```bash
maestro task create "Title" --description "..." --type feature --priority 1 --status in_progress
```

## Claim and start

Existing task:
```bash
maestro task next --json           # claim the next ready task
maestro task claim <id>            # claim a specific task
maestro task update <id> --status in_progress
```

## Optional: lock a contract for non-trivial work

`contract new` opens an editor by default, which hangs a non-interactive agent. Supply the draft via stdin or a template file instead:

```bash
cat <<'YAML' | maestro task contract new <id> --from - --session <id>
intent: >
  One to three sentences on what this task changes and why.
scope:
  filesExpected:
    - src/features/foo/**
  filesForbidden: []
doneWhen:
  - text: Describe the observable signal that proves the task is done.
    kind: manual
YAML

maestro task contract lock <id> --session <id>
```

To load a project-local template: `maestro task contract new <id> --from default --session <id>` (reads `.maestro/tasks/contract-templates/default.md`).

Contract amend/reopen/criteria verbs and verdict semantics live in `./reference/contracts.md`. Note: `contract amend` has no `--from` flag; it opens an editor. Use `--editor <cmd>` to drive it non-interactively.

## While working, keep resume state fresh

Only on meaningful change:
```bash
maestro task update <id> --current-state "..." --next-action "..."
maestro task update <id> --add-decision "keep API stable"
maestro task update <id> --remove-decision "old constraint"
```

Long-running work, heartbeat so the claim does not age out:
```bash
maestro task heartbeat <id>
```

Blockers:
```bash
maestro task block <blockerId> <blockedId...>
maestro task unblock <blockerId> <blockedId...>
```

## Complete with a receipt

```bash
maestro task update <id> --status completed \
  --reason "<one-line outcome>" \
  [--summary "<receipt summary>"] \
  [--surprise "<gotcha>"] \
  [--verified-by <name>] \
  [--strict]
```

`--reason` is persisted verbatim. Short, factual, no secrets. `--strict` blocks completion on a broken contract verdict.

## Discovery

```bash
maestro status --json
maestro task ready --json --compact --limit 5
maestro task show <id>
maestro task mine
maestro task stuck --older-than 4h
maestro task similar <id>
```

## Recovery

Stale or dead session:
```bash
maestro task claim <id> --stale-after 4h
maestro task release-owned <deadSessionId>
maestro task unclaim <id>
```

Local artifact pruning:
```bash
maestro task prune --dry-run
maestro task prune [--keep N] [--candidates-only|--continuations-only] [--all]
```

Deeper recovery patterns live in `./reference/recovery.md`. The full command surface lives in `./reference/commands.md`.

## Reference

- `./reference/plan-conversion.md`: longer examples mapping markdown plans to task batches
- `./reference/commands.md`: full CLI surface for `maestro task *`
- `./reference/contracts.md`: contract criteria, amend/reopen, verdicts, strict mode
- `./reference/recovery.md`: stuck / stale / release-owned flows
