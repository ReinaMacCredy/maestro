# Background Agent Manager Guide

Guidance for the orchestrator when managing 5+ concurrent background tasks during `/work` execution.

## Spawn Patterns

### Wave Spawning

Spawn workers in batches of 3-5, wait for completions, then spawn replacements. This prevents overwhelming the system and allows course correction between waves.

```
Wave 1: Spawn 3-4 workers for independent tasks
         Monitor via TaskList() polling
Wave 2: As Wave 1 workers complete, spawn replacements for next tasks
Wave 3: Continue until all tasks assigned
```

### Polling Pattern

Check `TaskList()` every 30 seconds for status updates when multiple workers are active. Look for:
- Newly completed tasks (verify results immediately)
- Stalled tasks (in_progress > 10 minutes with no heartbeat)
- Failed tasks (worker reported errors)

## Concurrency Limits

- **Maximum concurrent agents**: 5 (Claude Code's background task limit)
- **Recommended active workers**: 3-4 (leaves headroom for verification agents)
- **Reserve 1 slot** for ad-hoc spawns (build-fixer, critic)

## Failure Handling

When a background agent fails:

1. **Log the error** in the task description via `TaskUpdate(taskId, description: "...\\nError: {details}")`
2. **Do NOT retry the same agent immediately** -- investigate the failure first
3. **Spawn a replacement** with additional context about what went wrong
4. **If 3+ agents fail on the same task**, escalate to oracle for strategic guidance

## Worker Assignment Strategy

| Scenario | Strategy |
|----------|----------|
| All tasks independent | Spawn up to 4 workers simultaneously |
| Tasks have dependencies | Spawn independent tasks first, queue dependent ones |
| File contention risk | Assign overlapping-file tasks sequentially |
| Mix of simple + complex | Pair spark (simple) with kraken (complex) in same wave |

## Anti-Patterns

- **Do NOT** spawn all workers at once for 10+ task plans -- use waves
- **Do NOT** wait for all workers to finish before spawning more -- spawn as slots open
- **Do NOT** assign dependent tasks to parallel workers -- they will conflict
- **Do NOT** spawn oracle/critic alongside 4 workers -- reserve a slot
