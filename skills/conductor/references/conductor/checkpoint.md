# Checkpoint Facade

<!-- checkpoint v1 -->

Quick entry point for progress checkpointing operations.

## Primary Reference

→ [Progress Checkpointing](../../../beads/references/workflow.md#progress-checkpointing)

## Quick Reference

### When to Checkpoint

| Trigger | Action |
|---------|--------|
| Token budget > 70% | Proactive checkpoint |
| Token budget > 85% | Checkpoint + warn user |
| Token budget > 90% | Auto-checkpoint |
| Major milestone | Checkpoint notes |
| Hit a blocker | Capture what was tried |
| Task transition | Update before switching |

### Degradation Signals

→ [Degradation Signals](../../../beads/references/workflow.md#degradation-signals)

| Signal | Threshold |
|--------|-----------|
| `tool_repeat` | file_write: 3, bash: 3, search: 5, read: 10 |
| `backtrack` | 1 |
| `quality_drop` | 1 |
| `contradiction` | 1 |

**Rule:** 2+ signals → trigger compression

## See Also

- [Session Lifecycle](../../../design/references/session-lifecycle.md) - RECALL/REMEMBER
- [Remember Facade](remember.md) - Handoff protocol
