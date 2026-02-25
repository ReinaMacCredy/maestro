# Team Mode Protocol

## Overview

Team mode uses agent delegation to parallelize task execution. You (the skill runner) become the orchestrator. Workers (kraken/spark) handle implementation.

## Prerequisites

Your runtime must support agent delegation (teams, subagents, or equivalent).
- Claude Code: enable Agent Teams via `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`
- Other runtimes: use your native delegation mechanism

## Setup

### 1. Create Team

Create a worker team named "implement-{track_id}" (description: "Implementing track: {track_description}"). Use whatever team/delegation API your runtime provides.

### 2. Create Tasks from Plan

Parse `plan.md` and create one task per implementation item:

Create a task:
- **Subject**: "Phase {N} Task {M}: {task_title}"
- **Description**: Include context, spec reference, workflow (TDD or ship-fast), expected files to modify, and acceptance criteria (tests pass, coverage meets threshold, no lint errors).
- **Active form**: "Implementing {task_title}"

Set dependencies between tasks so that task M is blocked by task M-1.

**BR mirroring**: If `.beads/` exists and `metadata.json` has `beads_epic_id`, the orchestrator also mirrors task state to BR. Workers do NOT interact with `br` directly -- the orchestrator handles BR state changes after verifying worker output.

### 3. Spawn Workers

Spawn 2-3 workers based on track size:

**For TDD tasks** (features, new code):

Spawn a TDD worker (kraken) with the following prompt:

```
You are a TDD implementation worker on team 'implement-{track_id}'.

Your workflow:
1. Check the task list for available tasks (unblocked, no owner)
2. Claim one by setting owner to your name and status to in_progress
3. Read the task description for context
4. Follow TDD: write failing tests, implement to pass, refactor
5. Mark task completed
6. Check the task list for next available task
7. If no tasks available, notify the orchestrator

Project context:
- Workflow: .maestro/context/workflow.md
- Tech stack: .maestro/context/tech-stack.md
- Track spec: .maestro/tracks/{track_id}/spec.md
- Style guides: .maestro/context/code_styleguides/ (if exists)
- Priority context: .maestro/notepad.md (## Priority Context section, if exists)
```

**For quick-fix tasks** (bugs, small changes):

Spawn a quick-fix worker (spark) with the same workflow but without strict TDD requirement.

### 4. Worker Sizing

| Track Tasks | Workers | Types |
|-------------|---------|-------|
| 1-3 | 1 kraken | Single worker |
| 4-8 | 2 (1 kraken + 1 spark) | Mixed team |
| 8+ | 3 (2 kraken + 1 spark) | Full team |

## Orchestrator Responsibilities

### Monitor Progress

After spawning workers, periodically check the task list for available/completed work.

**BR supplementary monitoring**: If `beads_epic_id` exists, use `br ready --parent {epic_id} --json` alongside `TaskList` to see which BR issues are unblocked. This supplements (does not replace) Agent Teams task monitoring.

### Verify Completed Tasks

When a worker reports task completion:
1. Read the files they changed
2. Run the test suite
3. If verification passes:
   - Update plan.md: mark `[x] {sha}`
   - **BR mirror**: If `beads_epic_id` exists, also close the BR issue:
     ```bash
     br close {issue_id} --reason "sha:{sha7} | verified" --suggest-next --json
     ```
   - Commit: `git add . && git commit`
4. If verification fails:
   - Create a fix task
   - Assign to the worker or a build-fixer

### Handle Blockers

If a worker reports being blocked:
1. Read their message
2. Assess the blocker
3. Either:
   - Provide guidance via a message to the worker
   - Reassign to a different worker
   - Handle the blocker yourself (but NEVER edit code directly as orchestrator)

## Shutdown

After all tasks complete:

1. Request shutdown for each worker (e.g., "tdd-worker-1", "tdd-worker-2").
2. Wait for shutdown confirmations.
3. Tear down the worker team.

## Anti-patterns

| Don't | Do Instead |
|-------|-----------|
| Edit code directly as orchestrator | Delegate to workers |
| Spawn too many workers (>3) | Match worker count to task count |
| Skip verification | Always verify before committing |
| Let workers commit | Orchestrator commits after verification |
| Ignore worker messages | Respond promptly to unblock workers |
