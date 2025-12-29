# Graceful Fallback Pattern

Handle Agent Mail MCP failures without blocking work.

## Core Principle

**Coordination is optional; work completion is mandatory.**

Never let MCP unavailability block the epic.

## Fallback Hierarchy

```
Agent Mail available? 
  YES → Parallel dispatch with coordination
  NO  → Sequential execution via /conductor-implement
```

## Detection

Check Agent Mail availability at orchestration start:

```python
try:
    ensure_project(human_key=PROJECT_PATH)
    AGENT_MAIL_AVAILABLE = True
except McpUnavailable:
    AGENT_MAIL_AVAILABLE = False
    print("⚠️ Agent coordination unavailable - falling back to sequential")
```

## Failure Responses

### Orchestrator Level

| Operation | On Failure | Action |
|-----------|------------|--------|
| `ensure_project` | DEGRADE | Route to /conductor-implement |
| `register_agent` | Proceed | Log warning, continue without identity |
| `send_message` (epic start) | Proceed | Workers won't get thread context |
| Monitor loop fails | Proceed | Use bd CLI for status instead |

### Worker Level

| Operation | On Failure | Action |
|-----------|------------|--------|
| `register_agent` | Proceed | Work without identity |
| `file_reservation_paths` | Proceed | Risk of conflicts accepted |
| `send_message` (progress) | Proceed | Progress not tracked in thread |
| `send_message` (dep notify) | Escalate | Orchestrator must handle |

## Warning Format

**Orchestrator:**
```text
⚠️ Agent coordination unavailable - falling back to sequential execution
```

**Worker:**
```text
⚠️ Could not reserve files - proceeding without coordination
```

Keep warnings brief; don't alarm about optional features.

## Sequential Fallback

When Agent Mail unavailable, route to `/conductor-implement`:

```python
if not AGENT_MAIL_AVAILABLE:
    return conductor_implement(track_id)
```

This executes tasks sequentially in the main agent thread.

## Partial Failures

If Agent Mail becomes unavailable mid-orchestration:

1. **Active workers continue** - They have their beads, can complete
2. **Cross-track deps stall** - Workers can't notify each other
3. **Orchestrator intervenes** - Check bd CLI for completion, manually unblock

```python
# Fallback monitoring via bd CLI
while not all_complete:
    status = bash(f"bd list --parent={EPIC_ID} --status=open --json | jq 'length'")
    if status == "0":
        all_complete = True
    else:
        sleep(60)  # Longer interval without Agent Mail
```

## Recovery

If Agent Mail becomes available after partial failure:

1. Next MCP call succeeds automatically
2. Orchestrator resumes monitoring
3. Workers can send pending notifications
4. No need to retry failed calls

## Risk Acceptance

When proceeding without coordination:

| Risk | Mitigation |
|------|------------|
| File conflicts | Workers have scoped file patterns |
| Lost progress | bd CLI shows bead status |
| Stale workers | Check bd status directly |
| Lost handoffs | Session ends without context save |

## Implementation Notes

- Timeout on MCP calls: ~3 seconds
- Don't retry failed calls (wastes time)
- Log failures for debugging
- Prioritize work completion over coordination
