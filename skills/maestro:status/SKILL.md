---
name: maestro:status
description: "Show track progress overview with phase/task completion stats, next actions, and blockers."
allowed-tools: Read, Glob, Grep, Bash
disable-model-invocation: true
---

# Status -- Track Progress Overview

> Adapted from [Conductor](https://github.com/gemini-cli-extensions/conductor) for Claude Code.

Display a high-level overview of all tracks and detailed progress for in-progress tracks.

CRITICAL: You must validate the success of every tool call. If any tool call fails, halt immediately.

When using AskUserQuestion, immediately call the tool -- do not repeat the question in plain text.

---

## Step 1: Read Tracks Registry

```
Read(file_path: ".maestro/tracks.md")
```

If file doesn't exist:
- Report: "No tracks found. Run `/maestro:setup` then `/maestro:new-track` to get started."
- Stop.

## Step 2: Count Tracks by Status

Parse the registry and count, supporting both formats:

- New format: `- [ ] **Track: {description}**`
- Legacy format: `## [ ] Track: {description}`

Count by marker:
- `[ ]` -- New (pending)
- `[~]` -- In Progress
- `[x]` -- Complete

## Step 3: Detail In-Progress Tracks

For each track marked `[~]`:

1. Read its `plan.md`:
   ```
   Read(file_path: ".maestro/tracks/{track_id}/plan.md")
   ```

2. Parse phases and tasks:
   - Count `[ ]` (pending), `[~]` (in-progress), `[x]` (complete) per phase
   - Calculate overall completion percentage

3. Identify the next pending task (first `[ ]` in the plan)

4. Check for blockers:
   - Any task marked `[~]` for more than one phase indicates a stall
   - Any phase with failed verification noted

## Step 4: Assess Project Status

Using the data collected, compute a qualitative status:

- **Blocked** -- any task is explicitly blocked or a stall is detected (task `[~]` spanning multiple phases, failed verification)
- **Behind Schedule** -- completed tasks represent less than 25% of total tasks and at least one track is active
- **On Track** -- active tracks exist and no blockers detected
- **No Active Work** -- zero tracks marked `[~]`

## Step 5: Display Report

Format the output as:

```
## Tracks Overview

**Report generated**: {current date and time}
**Project status**: {On Track | Behind Schedule | Blocked | No Active Work}

| Status | Count |
|--------|-------|
| New    | {n}   |
| Active | {n}   |
| Done   | {n}   |

---

### Blockers

{If no blockers: "None detected."}
{Otherwise, list each blocked or stalled item:}
- [track_id] {task description} -- {reason: stalled / failed verification / explicitly blocked}

---

### Active: {track_description}
> ID: {track_id} | Type: {type}

**Phase 1: {title}** -- {completed}/{total} tasks [####----] {pct}%
**Phase 2: {title}** -- {completed}/{total} tasks [--------] {pct}%

**Next task**: {next_task_description}
**Run**: `/maestro:implement {track_id}`

---

### Recently Completed
- [x] {track_description} ({date})
```

## Step 6: Suggest Next Action

Based on the state:

- No tracks at all --> "Run `/maestro:setup` then `/maestro:new-track <description>`"
- All tracks complete --> "All tracks done. `/maestro:new-track` to start something new."
- Has pending tracks --> "Run `/maestro:implement {next_track}` to start."
- Has in-progress tracks --> "Run `/maestro:implement {active_track} --resume` to continue."
- Blocked --> "Resolve the blocker listed above before continuing."
