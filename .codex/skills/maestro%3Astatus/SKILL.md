---
name: maestro:status
description: "Interpret feature progress, detect problems, and recommend next actions based on maestro status output."
---

# Status -- Feature Progress & Next Actions

## Overview

Read the current feature state and translate it into actionable guidance. Status is the orientation layer -- run it at session start, after completing work, or when uncertain about what to do next.

**Core principle:** Status is not a passive display. It is a diagnostic. Detect problems, highlight what needs attention, and tell the user exactly what command to run next.

## When to Use

- Session start (always)
- After a task completes or fails
- When the user asks "what's next?" or "where are we?"
- Before starting any new work (to check for zombies, blockers, or stale state)

## Step 1: Gather State

Run `maestro status` (or `maestro status --feature <name>` for a specific feature).

If no active feature is set:
- Report: "No active feature. Run `maestro feature-active <name>` to set one, or `maestro feature-list` to see available features."
- If no features exist at all: "No features found. Run `maestro feature-create <name>` to start."
- Stop.

The status output contains these fields:
- **feature**: name and status (`planning` | `approved` | `executing` | `completed`)
- **plan**: exists (yes/no), approved (yes/no), comment count
- **tasks**: total, pending, in_progress, done counts, plus individual task list with statuses
- **zombies**: stale in_progress tasks where the session is missing or expired
- **blocked**: tasks blocked by unfinished dependencies
- **context**: file count and byte total for saved context
- **nextAction**: the system's recommended next step

## Step 2: Identify the Feature Phase

Map the status output to one of four phases. This determines what to emphasize in the report.

| Feature Status | Plan     | Tasks      | Phase         | Focus                              |
|----------------|----------|------------|---------------|------------------------------------|
| `planning`     | none     | 0          | Discovery     | Gathering requirements, exploring  |
| `planning`     | draft    | 0          | Planning      | Refining plan, addressing comments |
| `approved`     | approved | 0          | Pre-Execution | Need to sync tasks from plan       |
| `approved`     | approved | >0 pending | Execution     | Starting and running tasks         |
| `executing`    | approved | mixed      | Execution     | Active work in progress            |
| `completed`    | any      | all done   | Completion    | Review and wrap-up                 |

**Phase determines what sections to show:**

- **Discovery**: Emphasize context files, suggest brainstorming or design skills, de-emphasize tasks (none exist).
- **Planning**: Emphasize plan status and comments. If comments exist, highlight them -- they may contain unresolved feedback.
- **Pre-Execution**: Single clear action: run `maestro task-sync`.
- **Execution**: Full task breakdown with progress, conditions, and next action. This is the most detailed phase.
- **Completion**: Brief summary. Show done count. Suggest `maestro feature-complete`.

## Step 3: Detect Conditions Requiring Attention

Scan the status output for these conditions. Each one requires a specific callout in the report.

| Condition              | Detection Signal                           | Severity | Action                                                                                   |
|------------------------|--------------------------------------------|----------|------------------------------------------------------------------------------------------|
| Zombie (stale task)    | `zombies` list is non-empty                | High     | `maestro task-start --feature <f> --task <id> --force` to recover                        |
| Blocked task           | Task status is `blocked`                   | High     | Review blocker, then `maestro task-start --task <id> --continue-from blocked --decision`  |
| Failed task            | Task status is `failed`                    | Medium   | `maestro task-update --feature <f> --task <id> --status pending` to reset                |
| Partial task           | Task status is `partial`                   | Medium   | `maestro task-start --feature <f> --task <id> --continue-from partial` to resume         |
| Unreviewed comments    | `plan.commentCount > 0` and plan is draft  | Medium   | Read comments with `maestro plan-read`, address feedback, revise plan                    |
| No tasks synced        | Plan approved but `tasks.total == 0`       | Low      | `maestro task-sync --feature <name>` to generate tasks                                   |
| All pending, dep-blocked | All tasks pending, `blocked` map non-empty | Low      | Check dependency chain -- something upstream may need manual intervention                |

**Severity determines formatting:**
- **High**: Mark with `[!]` prefix. These block progress and must be resolved first.
- **Medium**: Mark with `[~]` prefix. These represent interrupted work that should be resumed.
- **Low**: Mark with `-->` prefix. These are informational next steps.

## Step 4: Present the Status Report

Format the report using phase-aware rules. See `reference/output-templates.md` for concrete examples of every phase and condition.

**General formatting rules:**

1. **Lead with the headline**: Feature name, phase, and overall health in one line.
2. **Show problems first**: Any High or Medium conditions appear before the task list.
3. **Task list**: Show all tasks with status markers. Use alignment for readability.
4. **Progress bar**: For execution phase, show completion as `done/total` with percentage.
5. **Next action**: Always end with the specific command to run next.
6. **Suppress empty sections**: Do not show "Blockers: None" or "Zombies: None". Only show sections that have content.

**Phase-specific rules:**

- **Discovery**: Show context file count if any. Suggest `maestro:brainstorming` or `maestro:design` skills.
- **Planning**: Show plan status prominently. If comments exist, show count and suggest `maestro plan-read`. If plan is draft, suggest `maestro plan-write` or `maestro plan-approve`.
- **Execution**: Full task table. Group by status: in_progress first, then partial/blocked/failed, then pending, then done (collapsed if many).
- **Completion**: Brief summary. Show done count. Suggest `maestro feature-complete`.

## Step 5: Recommend Next Action

The `nextAction` field from `maestro status` provides the primary recommendation. Use it as the base, then layer on context-aware guidance.

**Enhancement rules based on phase and conditions:**

| Phase     | Condition          | Beyond nextAction                                                          |
|-----------|--------------------|----------------------------------------------------------------------------|
| Discovery | No plan            | Suggest loading `maestro:brainstorming` skill before writing plan          |
| Planning  | Comments exist     | "Address the N comments before seeking approval"                           |
| Planning  | No comments        | "Plan looks clean -- run `maestro plan-approve` when ready"                |
| Execution | Zombie detected    | "Recover the stale task BEFORE starting new work"                          |
| Execution | Blocked task       | "Read the blocker report: `maestro task-report-read --task <id>`"          |
| Execution | Multiple runnable  | "N tasks are ready. Pick based on priority or dependencies."               |
| Execution | All done           | "All tasks complete. Review implementation, then `maestro feature-complete`"|
| Execution | Failed + runnable  | "Reset the failed task or skip it and continue with runnable tasks"        |
| Any       | Zero context files | "Consider saving key decisions: `maestro context-write`"                   |

**Compound conditions**: When multiple conditions coexist (e.g., zombie + blocked), prioritize by severity. Address the highest-severity condition first in the recommendation.

## Relationship to Other Commands and Skills

Status is the observability layer across the maestro workflow:

| Command / Skill         | Relationship to Status                                     |
|-------------------------|------------------------------------------------------------|
| `maestro feature-create`| Creates the feature that status reads                      |
| `maestro plan-write`    | Status tracks plan existence and approval state            |
| `maestro plan-approve`  | Status detects approved plans and suggests task-sync       |
| `maestro task-sync`     | Generates tasks that status tracks                         |
| `maestro task-start`    | Status detects in_progress, partial, blocked, failed tasks |
| `maestro task-finish`   | Status updates done count and recalculates next action     |
| `maestro:brainstorming` | Status suggests this skill during Discovery phase          |
| `maestro:design`        | Status suggests this skill for complex features            |
| `maestro:implement`     | Load during Execution phase alongside status               |

Run `maestro status` before and after every significant action to stay oriented.
