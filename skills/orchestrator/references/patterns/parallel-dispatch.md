# Parallel Dispatch Pattern

Execute independent tracks concurrently using Task tool with Agent Mail coordination.

## When to Use

- Plan.md has Track Assignments section
- 2+ tracks with independent file scopes
- Epic can be decomposed into parallel work streams

## Prerequisites

- Beads filed from plan (`fb` command)
- Agent Mail CLI available (HALT if unavailable)
- Track Assignments in plan.md

## Pattern

### Track Assignment Table

```markdown
## Track Assignments

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/design/** | 1.2.3 |
| 3 | RedStone | 3.1.*, 4.* | conductor/CODEMAPS/** | 2.2.2 |
```

### Dispatch Flow

```
1. Parse Track Assignments from plan.md
2. Map tasks to bead IDs via metadata.json.beads.planTasks
3. Initialize Agent Mail (ensure-project, register-agent)
4. Reserve files for all tracks
5. Spawn workers via Task() (parallel)
6. Monitor via fetch-inbox, search-messages
7. Handle cross-track blockers
8. Verify completion, close epic
```

## Worker Spawn

Each track gets a Task() call with:

```python
Task(
  description=f"Worker {agent}: Track {n} - {description}",
  prompt=worker_prompt.format(
    AGENT_NAME=agent,
    TRACK_N=n,
    TASK_LIST=tasks,
    BEAD_LIST=beads,
    FILE_SCOPE=scope,
    DEPENDS_ON=deps
  )
)
```

All Task() calls are made in a single assistant message for parallel execution.

## File Scope Isolation

Each track has exclusive file scope:

- Track 1: `skills/orchestrator/**`
- Track 2: `skills/design/**`
- Track 3: `conductor/CODEMAPS/**`

Workers reserve their scope via `bun toolboxes/agent-mail/agent-mail.js file-reservation-paths`.

## Cross-Track Dependencies

When Track 2 depends on Task 1.2.3 from Track 1:

1. Worker 2 checks inbox before starting blocked beads
2. Worker 1 sends `[DEP] 1.2.3 COMPLETE` when done
3. Worker 2 receives notification, proceeds

If dependency times out (30 min), orchestrator intervenes.

## Agent Mail Failure

If Agent Mail unavailable:

```text
❌ Cannot proceed: Agent Mail required for parallel execution
```

HALT execution. Do not fall back to sequential - user must fix Agent Mail availability first.

## See Also

- [workflow.md](../workflow.md) - Full 6-phase protocol
- [worker-prompt.md](../worker-prompt.md) - Worker template
- [graceful-fallback.md](graceful-fallback.md) - Failure handling
