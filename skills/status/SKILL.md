---
name: status
description: "Shows current Maestro state including plans, drafts, active tasks, and wisdom files. Use when checking progress and session health."
allowed-tools: Read, Bash, Glob, Grep, TaskList
disable-model-invocation: true
---

# Maestro Status

Report the current state of all Maestro artifacts.
Do not collect or summarize execution timeline/performance telemetry here; that belongs to `/trace`.

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

### 1.6. Context

List all files in `.maestro/context/`:
- File name
- First line (title)
- Last modified date

If empty, report "No project context. Run `/setup` to create."

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

### 4.5. Research

List all files in `.maestro/research/`:
- File name
- First line (title)
- Last modified date

If empty, report "No research sessions. Run `/research <topic>` to start one."

### 4.6. Notepad

Check if `.maestro/notepad.md` exists:
- If it exists, show each section header and bullet count:
  - `## Priority Context` — N items
  - `## Working Memory` — N items
  - `## Manual` — N items
- If any priority items exist, display them (these are injected at session start)

If file doesn't exist, report "No notepad. Run `/note <content>` to start."

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

### 6.5. PSM Sessions

Check PSM session state:

```bash
ls -la ~/.maestro-psm/sessions.json 2>/dev/null
```

If `~/.maestro-psm/sessions.json` exists, report active session count from `.sessions` entries:

```bash
jq '.sessions | length' ~/.maestro-psm/sessions.json
```

If missing, report "No PSM sessions state found."

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
| No context files exist | "Run `/setup` to scaffold project context (product, tech stack, guidelines)." |
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
| Context | N | <name> |
| Drafts | N | <name> |
| Tasks | N (X active) | - |
| Handoffs | N | <name> |
| Wisdom | N | <name> |
| Research | N | <name> |
| Notepad | Present/Absent | N priority items |
| Worktrees | N active | - |
| Teams | N active | - |
| PSM Sessions | Present/Absent | N active |
```
