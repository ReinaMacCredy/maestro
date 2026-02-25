---
name: maestro-status
description: "Show track progress overview with phase/task completion stats, next actions, and blockers."
---

# Status -- Track Progress Overview

> This skill is CLI-agnostic. It works with Claude Code, Codex, Amp, or any AI coding assistant.

Display a high-level overview of all tracks and detailed progress for in-progress tracks.

Validate the result of every operation. If any step fails, halt and report the failure before continuing.

---

## Step 1: Read Tracks Registry

Read `.maestro/tracks.md`.

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

1. Read `.maestro/tracks/{track_id}/metadata.json` and `.maestro/tracks/{track_id}/plan.md`.

2. **BR-enhanced path**: If `metadata.json` has `beads_epic_id`:
   - Use `br epic status --json` for overall progress stats (open/closed/total)
   - Use `br list --status open --label "phase:{N}-{kebab}" --json` for per-phase open counts
   - Use `bv -robot-insights -format json` for graph health (cycles, bottlenecks, stale issues)
   - Use `bv -robot-next -format json` for the top recommended next action
   - Falls back to plan.md parsing if any BR/BV command fails

3. **Legacy path** (no `beads_epic_id`): Parse phases and tasks from plan.md:
   - Count `[ ]` (pending), `[~]` (in-progress), `[x]` (complete) per phase
   - Calculate overall completion percentage

4. Identify the next pending task (first `[ ]` in the plan, or from `bv -robot-next`)

5. Check for blockers:
   - Any task marked `[~]` for more than one phase indicates a stall
   - Any phase with failed verification noted

## Step 4: Assess Project Status

Using the data collected, compute a qualitative status:

- **Blocked** -- any task is explicitly blocked or a stall is detected (task `[~]` spanning multiple phases, failed verification)
- **Behind Schedule** -- completed tasks represent less than 25% of total tasks and at least one track is active
- **On Track** -- active tracks exist and no blockers detected
- **No Active Work** -- zero tracks marked `[~]`

**BR health check**: If any track has `beads_epic_id`, also check `bv -robot-insights -format json` for:
- Dependency cycles (critical blocker)
- Stale issues (warning)
- Bottleneck nodes (informational)

Include health signals in the report under a "Health" subsection when available.

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

---

## Relationship to Other Commands

Recommended workflow:

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute the implementation
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- **You are here.** Check progress across all tracks
- `/maestro:revert` -- Undo implementation if needed
- `/maestro:note` -- Capture decisions and context to persistent notepad

Status is the observability layer across all maestro commands. It reads tracks created by `/maestro:new-track`, progress from `/maestro:implement`, and state changes from `/maestro:revert`. Use it anytime to orient yourself on what to do next.
