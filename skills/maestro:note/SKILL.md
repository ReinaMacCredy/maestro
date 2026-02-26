---
name: maestro-note
description: "Capture decisions, constraints, and context to persistent notepad. Priority notes are injected into every session and implementation run."
argument-hint: "<content> [--priority|--manual|--show|--prune|--clear <section>]"
---

# Note -- Persistent Working Memory

Manage notes that persist across sessions. Priority context is injected at session start and during `/maestro:implement` execution.

## Arguments

`$ARGUMENTS`

- `<content>`: Text to add (default: appends to Working Memory)
- `--priority <content>`: Add to Priority Context (injected into sessions and implementation)
- `--manual <content>`: Add to Manual section (persistent until manually removed)
- `--show`: Display full notepad contents
- `--prune`: Remove stale entries from Working Memory
- `--clear <section>`: Clear a section (`priority`, `working`, `all`)

---

## Step 1: Parse Arguments

Extract the flag and content from `$ARGUMENTS`:

| Input | Flag | Target Section |
|-------|------|---------------|
| `<content>` (no flag) | default | Working Memory |
| `--priority <content>` | priority | Priority Context |
| `--manual <content>` | manual | Manual |
| `--show` | show | (read-only) |
| `--prune` | prune | Working Memory |
| `--clear <section>` | clear | specified section |

## Step 2: Ensure Notepad Exists

If `.maestro/notepad.md` does not exist, create it:

```markdown
# Notepad
## Priority Context

## Working Memory

## Manual
```

Also ensure `.maestro/` directory exists.

## Step 3: Execute

**For add commands** (default, `--priority`, `--manual`):

1. Read `.maestro/notepad.md`
2. Find the target section header (`## Priority Context`, `## Working Memory`, or `## Manual`)
3. Append `- <content>` after the section header (before the next `##` section)
4. Write the updated file

**For `--show`**:

1. Read and display `.maestro/notepad.md`
2. If file does not exist: "No notepad found. Use `/maestro:note <content>` to start."

**For `--prune`**:

1. Read `.maestro/notepad.md`
2. Review each bullet in `## Working Memory`
3. Remove items that appear stale or resolved
4. Keep `## Priority Context` and `## Manual` intact
5. Show what was removed and what was kept
6. If uncertain about an item, ask the user

**For `--clear`**:

1. Parse the section argument: `priority`, `working`, or `all`
2. If clearing Priority Context or all: confirm with the user first
3. Remove all bullets from the specified section(s)
4. Keep section headers intact

## Step 4: Confirm

After any write operation, display the updated section to confirm the change was applied correctly.

---

## Section Contracts

| Section | Written By | Read By | Persistence |
|---------|-----------|---------|-------------|
| Priority Context | User via `--priority` | `session-start.sh`, `maestro:implement` Step 3.8, team-mode worker prompts | Until manually cleared |
| Working Memory | Default `/maestro:note`, `maestro:implement` Step 6a.8.5 (auto-capture) | Sessions, prune | Pruned periodically |
| Manual | User via `--manual` | Sessions | Until manually cleared |

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- **You are here.** Capture decisions and context to persistent notepad

Note is the cross-cutting memory layer. Priority context is automatically loaded by `/maestro:implement` at execution start (Step 3.8) and injected into worker prompts in team mode. Working Memory accumulates insights from both manual notes and auto-capture during implementation (Step 6a.8.5). Use `/maestro:note --prune` periodically to keep working memory relevant.
