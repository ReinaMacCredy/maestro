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

### 5. Handoffs

List all files in `.maestro/handoff/`:
- File name
- Topic and status from JSON content

If empty, report "No active handoffs."

If any handoff has `status: "designing"`, highlight it: "Design in progress for {topic} — started {timestamp}."

### 6. Teams

Check for active teams:
```bash
ls ~/.claude/teams/ 2>/dev/null
```

Report any active team directories.

### 7. Next Steps

Based on the state discovered above, suggest the most relevant next action:

| State | Suggestion |
|-------|------------|
| Plans exist + no active tasks | "Ready to execute. Run `/work` to start." |
| Wisdom exists + no active tasks | "Previous cycle complete. Run `/design` for next iteration or `/review` to verify." |
| Active teams present | "Workers may be running. Run `/reset` if stuck." |
| Handoff with status "designing" | "Design in progress. Run `/design` to continue or `/reset` to clean up." |
| Drafts exist + no plans | "Interview was interrupted. Run `/design` to continue or `/reset` to start fresh." |
| Empty state (no plans, drafts, tasks, wisdom) | "Get started: Run `/setup-check`, then `/design <your request>`." |

Display all matching suggestions. Multiple states can apply simultaneously.

## Output

End with a summary table:
```
## Maestro Status

| Artifact | Count | Latest |
|----------|-------|--------|
| Plans | N | <name> |
| Drafts | N | <name> |
| Tasks | N (X active) | - |
| Handoffs | N | <name> |
| Wisdom | N | <name> |
| Teams | N active | - |
```
