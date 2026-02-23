---
name: note
description: "Manages persistent working-memory notes across sessions. Use when you need to store priority context, decisions, or reminders."
argument-hint: "<content> [--priority|--manual|--show|--prune|--clear]"
allowed-tools: Read, Write, Edit, Bash, Glob, AskUserQuestion
disable-model-invocation: true
---

# Note — Persistent Working Memory

> Manage notes that persist across Claude Code sessions. Priority context is injected at session start.

## Storage

All notes live in `.maestro/notepad.md` with three sections:

```markdown
# Notepad
## Priority Context
[Injected into every session start — use for critical reminders]

## Working Memory
[Accumulated context from sessions — auto-managed]

## Manual
[User-written notes that persist until manually removed]
```

## Commands

### Default (no flag): Add to Working Memory

```
/note Fix: auth middleware was missing token refresh check
```

Appends the content as a bullet to `## Working Memory`.

### `--priority`: Add to Priority Context

```
/note --priority Fix auth before deploying to prod
```

Appends the content as a bullet to `## Priority Context`. This section is read by `session-start.sh` and injected into every new session.

### `--manual`: Add to Manual Notes

```
/note --manual API rate limit is 100 req/min per key
```

Appends the content as a bullet to `## Manual`.

### `--show`: Display Notepad

```
/note --show
```

Reads and displays the full notepad contents.

### `--prune`: Prune Working Memory

```
/note --prune
```

Removes entries from `## Working Memory` that are no longer relevant. Keeps `## Priority Context` and `## Manual` intact. Uses judgment to remove stale items — ask the user if uncertain.

### `--clear`: Clear a Section

```
/note --clear priority
/note --clear working
/note --clear all
```

Clears the specified section (or all sections). Asks for confirmation before clearing `## Priority Context` or all.

## Workflow

### Step 1: Parse Arguments

Extract the flag (if any) and the content from the user's input.

- No flag → default to Working Memory
- `--priority` → Priority Context
- `--manual` → Manual
- `--show` → display only
- `--prune` → prune Working Memory
- `--clear <section>` → clear section

### Step 2: Ensure Notepad Exists

If `.maestro/notepad.md` doesn't exist, create it with the template:

```markdown
# Notepad
## Priority Context

## Working Memory

## Manual
```

Also ensure `.maestro/` directory exists.

### Step 3: Execute Command

**For add commands** (`default`, `--priority`, `--manual`):
1. Read the current notepad
2. Find the target section header
3. Append `- <content>` after the section header (before the next section)
4. Write the updated notepad

**For `--show`**:
1. Read and display the notepad
2. If it doesn't exist, say "No notepad found. Use `/note <content>` to start."

**For `--prune`**:
1. Read the notepad
2. Review each bullet in `## Working Memory`
3. Remove items that appear stale or resolved
4. Show what was removed

**For `--clear`**:
1. Confirm with the user (unless clearing Working Memory only)
2. Remove all bullets from the specified section(s)
3. Keep section headers intact

### Step 4: Confirm

After any write operation, show the updated section to confirm the change.

## Section Contracts

| Section | Written by | Read by | Persistence |
|---------|-----------|---------|-------------|
| Priority Context | User via `--priority` | `session-start.sh` | Until manually cleared |
| Working Memory | Default `/note` | Sessions, prune | Pruned periodically |
| Manual | User via `--manual` | Sessions | Until manually cleared |
