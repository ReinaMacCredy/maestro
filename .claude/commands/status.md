---
name: status
description: Show current Maestro state — plans, drafts, active tasks, and wisdom files.
allowed-tools: Read, Bash, Glob, Grep, TaskList
---

# Maestro Status

Report the current state of all Maestro artifacts.

## Sections

### 1. Plans

List all files in `.maestro/plans/`:
- File name
- First line (title)
- Last modified date

If empty, report "No plans found. Run /design to create one."

### 2. Drafts

List all files in `.maestro/drafts/`:
- File name
- First line (title)
- Last modified date

If empty, report "No active drafts."

### 3. Active Tasks

Run `TaskList` to check for any active task lists:
- Show task count by status (pending, in_progress, completed)
- List any blocked tasks

If no tasks, report "No active task lists."

### 4. Wisdom

List all files in `.maestro/wisdom/`:
- File name
- First line (title)
- File size

If empty, report "No wisdom accumulated yet. Complete a /work cycle to start learning."

### 5. Teams

Check for active teams:
```bash
ls ~/.claude/teams/ 2>/dev/null
```

Report any active team directories.

## Output

End with a summary table:
```
## Maestro Status

| Artifact | Count | Latest |
|----------|-------|--------|
| Plans | N | <name> |
| Drafts | N | <name> |
| Tasks | N (X active) | - |
| Wisdom | N | <name> |
| Teams | N active | - |
```
