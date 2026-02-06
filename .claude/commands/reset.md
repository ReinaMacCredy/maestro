---
name: reset
description: Clean stale Maestro state — remove old drafts, orphaned teams, and stale task directories.
allowed-tools: Read, Write, Bash, Glob
---

# Maestro Reset

Clean up stale Maestro state. This is a maintenance command — use when things get stuck.

## What Gets Cleaned

### 1. Stale Drafts

List all draft files in `.maestro/drafts/`:
```
Glob(".maestro/drafts/*.md")
```

For each draft, show the file name and first line. Ask the user which to remove (or all).

### 2. Orphaned Team Directories

Check for team directories that may be leftover from interrupted sessions:
```bash
ls -la ~/.claude/teams/ 2>/dev/null
```

For each team directory, check if it has active members. Report orphaned teams.

### 3. Stale Task Directories

Check for task directories:
```bash
ls -la ~/.claude/tasks/ 2>/dev/null
```

Report any task directories that don't correspond to active teams.

## Safety Rules

- **NEVER delete plans** — Plans in `.maestro/plans/` are preserved
- **NEVER delete wisdom** — Wisdom in `.maestro/wisdom/` is preserved
- **Confirm before deleting** — Show what will be removed and ask for confirmation
- **Report what was cleaned** — List every file/directory removed

## Process

1. Scan all three areas (drafts, teams, tasks)
2. Report findings to the user
3. Wait for confirmation before removing anything
4. Remove confirmed items
5. Report cleanup results

## Output

End with:
```
## Reset Complete

### Cleaned
- [N] draft files removed
- [N] orphaned team directories removed
- [N] stale task directories removed

### Preserved
- [N] plans in .maestro/plans/
- [N] wisdom files in .maestro/wisdom/
```
