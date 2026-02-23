---
name: reset
description: "Cleans stale Maestro state by removing old drafts, orphaned teams, and stale task directories. Use when the local Maestro state is inconsistent or cluttered."
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

### 3. Stale Handoff Files

List all handoff files in `.maestro/handoff/`:
```
Glob(".maestro/handoff/*.json")
```

For each handoff file, read the JSON and show the topic, status, and start time. Report handoff files that may be from interrupted design sessions (especially those with `status: "designing"`).

### 4. Stale Task Directories

Check for task directories:
```bash
ls -la ~/.claude/tasks/ 2>/dev/null
```

Report any task directories that don't correspond to active teams.

### 5. Archived Plans

List all archived plan files in `.maestro/archive/`:
```
Glob(".maestro/archive/*.md")
```

For each archived plan, show the file name and first line (title). Ask the user which to remove (or all, or none). This is the ONLY way archived plans get deleted — it requires explicit user confirmation.

### 6. Orphaned Worktrees

Check for Maestro worktrees that may be leftover from interrupted sessions:
```bash
git worktree list --porcelain
```

Filter for worktrees on `maestro/*` branches. Cross-reference with handoff files in `.maestro/handoff/`:
- A worktree is **orphaned** if:
  - No corresponding handoff file exists, OR
  - The corresponding handoff has `status: "complete"` (session finished but worktree was not cleaned up)

For each orphaned worktree:
- Show the path, branch name, and any available context from handoff files
- Ask the user for confirmation before removing:

```bash
git worktree remove "<path>"
```

After removing the worktree, optionally offer to delete the associated branch (with separate confirmation):

```bash
git branch -D "maestro/<slug>"
```

**Safety**: Never auto-remove worktrees. Always require explicit user confirmation for both worktree removal and branch deletion.

## Safety Rules

- **Plans in `.maestro/plans/` are NEVER deleted.** Archived plans in `.maestro/archive/` may be deleted with user confirmation.
- **NEVER delete wisdom** — Wisdom in `.maestro/wisdom/` is preserved
- **Confirm before deleting** — Show what will be removed and ask for confirmation
- **Report what was cleaned** — List every file/directory removed

## Process

1. Scan all six areas (drafts, handoffs, teams, tasks, archive, worktrees)
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
- [N] handoff files removed
- [N] orphaned team directories removed
- [N] stale task directories removed
- [N] archived plans removed
- [N] orphaned worktrees removed

### Preserved
- [N] plans in .maestro/plans/
- [N] wisdom files in .maestro/wisdom/
- [N] archived plans in .maestro/archive/
```
