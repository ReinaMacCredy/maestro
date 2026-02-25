# Team Mode Protocol

## Overview

Team mode uses Agent Teams to parallelize task execution. You (the skill runner) become the orchestrator. Workers (kraken/spark) handle implementation.

## Prerequisites

Agent Teams must be enabled:
```json
// ~/.claude/settings.json
{ "env": { "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1" } }
```

## Setup

### 1. Create Team

```
TeamCreate(
  team_name: "implement-{track_id}",
  description: "Implementing track: {track_description}"
)
```

### 2. Create Tasks from Plan

Parse `plan.md` and create one task per implementation item:

```
TaskCreate(
  subject: "Phase {N} Task {M}: {task_title}",
  description: "## Context\n{task_description}\n\n## Spec Reference\n{relevant_spec_section}\n\n## Workflow\n{tdd_or_shipfast}\n\n## Files\n{expected_files_to_modify}\n\n## Acceptance\n- Tests pass\n- Coverage meets threshold\n- No lint errors",
  activeForm: "Implementing {task_title}"
)
```

Set dependencies:
```
TaskUpdate(taskId: "{task_M_id}", addBlockedBy: ["{task_M-1_id}"])
```

### 3. Spawn Workers

Spawn 2-3 workers based on track size:

**For TDD tasks** (features, new code):
```
Task(
  subagent_type: "kraken",
  name: "tdd-worker-{n}",
  team_name: "implement-{track_id}",
  model: "sonnet",
  prompt: "You are a TDD implementation worker on team 'implement-{track_id}'.

Your workflow:
1. Check TaskList for available tasks (unblocked, no owner)
2. Claim one with TaskUpdate (set owner to your name, status to in_progress)
3. Read the task description for context
4. Follow TDD: write failing tests, implement to pass, refactor
5. Mark task completed with TaskUpdate
6. Check TaskList for next available task
7. If no tasks available, notify the orchestrator via SendMessage

Project context:
- Workflow: .maestro/context/workflow.md
- Tech stack: .maestro/context/tech-stack.md
- Track spec: .maestro/tracks/{track_id}/spec.md
- Style guides: .maestro/context/code_styleguides/ (if exists)"
)
```

**For quick-fix tasks** (bugs, small changes):
```
Task(
  subagent_type: "spark",
  name: "fix-worker-{n}",
  team_name: "implement-{track_id}",
  model: "sonnet",
  prompt: "You are a quick-fix worker on team 'implement-{track_id}'.
{same workflow as above but without strict TDD requirement}"
)
```

### 4. Worker Sizing

| Track Tasks | Workers | Types |
|-------------|---------|-------|
| 1-3 | 1 kraken | Single worker |
| 4-8 | 2 (1 kraken + 1 spark) | Mixed team |
| 8+ | 3 (2 kraken + 1 spark) | Full team |

## Orchestrator Responsibilities

### Monitor Progress

After spawning workers, periodically check:
```
TaskList()
```

### Verify Completed Tasks

When a worker reports task completion:
1. Read the files they changed
2. Run the test suite
3. If verification passes:
   - Update plan.md: mark `[x] {sha}`
   - Commit: `git add . && git commit`
4. If verification fails:
   - Create a fix task
   - Assign to the worker or a build-fixer

### Handle Blockers

If a worker reports being blocked:
1. Read their message
2. Assess the blocker
3. Either:
   - Provide guidance via SendMessage
   - Reassign to a different worker
   - Handle the blocker yourself (but NEVER edit code directly as orchestrator)

## Shutdown

After all tasks complete:

```
SendMessage(type: "shutdown_request", recipient: "tdd-worker-1")
SendMessage(type: "shutdown_request", recipient: "tdd-worker-2")
// Wait for shutdown confirmations
TeamDelete()
```

## Anti-patterns

| Don't | Do Instead |
|-------|-----------|
| Edit code directly as orchestrator | Delegate to workers |
| Spawn too many workers (>3) | Match worker count to task count |
| Skip verification | Always verify before committing |
| Let workers commit | Orchestrator commits after verification |
| Ignore worker messages | Respond promptly to unblock workers |
