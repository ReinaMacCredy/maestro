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

### 1.5. Archive

List all files in `.maestro/archive/`:
- File name
- First line (title)
- Last modified date

If empty, report "No archived plans."

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

### 5.5. Worktrees

Check for active Maestro worktrees:
```bash
git worktree list --porcelain
```

Filter for worktrees on `maestro/*` branches (these are Maestro-created worktrees).

For each Maestro worktree, report:
- Worktree path
- Branch name
- Cross-reference with handoff files (check if any `.maestro/handoff/*.json` has matching `worktree_path` or `worktree_branch`)

If no Maestro worktrees found, report "No active Maestro worktrees."

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
| Plans exist + no active tasks | Cross-reference `.maestro/handoff/*.json` for a handoff with `status: "complete"` whose `plan_destination` matches an existing plan. If found: "Ready to execute: **{plan title}**. Run `/work` to start, or `claude "/work"` for a fresh session." If no matching handoff: "Ready to execute. Run `/work` to start." |
| Wisdom exists + no active tasks | "Previous cycle complete. Run `/design` for next iteration or `/review` to verify." |
| Active teams present | "Workers may be running. Run `/reset` if stuck." |
| Worktrees exist + no active tasks | "Worktrees may be from completed sessions. Run `/reset` to clean up." |
| Handoff with status "designing" | "Design in progress. Run `/design` to continue or `/reset` to clean up." |
| Drafts exist + no plans | "Interview was interrupted. Run `/design` to continue or `/reset` to start fresh." |
| Archive has items + no active plans | "Previous plans archived. Run `/design` for next iteration." |
| Empty state (no plans, drafts, tasks, wisdom) | "Get started: Run `/setup-check`, then `/design <your request>`." |

Display all matching suggestions. Multiple states can apply simultaneously.

## Output

End with a summary table:
```
## Maestro Status

| Artifact | Count | Latest |
|----------|-------|--------|
| Plans | N | <name> |
| Archive | N | <name> |
| Drafts | N | <name> |
| Tasks | N (X active) | - |
| Handoffs | N | <name> |
| Wisdom | N | <name> |
| Worktrees | N active | - |
| Teams | N active | - |
```
