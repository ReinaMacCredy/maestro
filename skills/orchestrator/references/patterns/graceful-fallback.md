# Agent Mail Availability Policy

Agent Mail is **required** for parallel orchestration. No fallback.

## Core Principle

**Coordination is mandatory for parallel execution.**

Without Agent Mail, parallel workers cannot coordinate safely.

## Availability Check

```
Agent Mail available? 
  YES → Parallel dispatch with coordination
  NO  → HALT (cannot proceed without coordination)
```

## Detection

Check Agent Mail availability at orchestration start:

```python
try:
    result = health_check(reason="Orchestrator preflight")
    if not result.healthy:
        raise McpUnavailable("Agent Mail not healthy")
    ensure_project(human_key=PROJECT_PATH)
    AGENT_MAIL_AVAILABLE = True
except McpUnavailable:
    # CRITICAL: Do NOT fall back to sequential
    print("❌ HALT: Agent Mail unavailable - cannot orchestrate")
    print("   Parallel execution requires Agent Mail for:")
    print("   - Worker registration and identity")
    print("   - File reservation to prevent conflicts")
    print("   - Cross-track dependency notifications")
    print("   - Progress monitoring and blocker resolution")
    return {"status": "HALTED", "reason": "Agent Mail unavailable"}
```

## Failure Responses

### Orchestrator Level

| Operation | On Failure | Action |
|-----------|------------|--------|
| `health_check` | HALT | Cannot proceed with orchestration |
| `ensure_project` | HALT | Cannot proceed with orchestration |
| `register_agent` | HALT | Workers need identity for coordination |
| `send_message` (epic start) | HALT | Workers need thread context |
| Monitor loop fails | Log + Retry | Use `fetch_inbox` with backoff |

### Worker Level

| Operation | On Failure | Action |
|-----------|------------|--------|
| `macro_start_session` | HALT | Cannot work without session |
| `file_reservation_paths` | HALT | Cannot risk file conflicts |
| `send_message` (progress) | Log + Continue | Non-critical |
| `send_message` (dep notify) | HALT | Must notify dependencies |

## Error Format

**Orchestrator:**
```text
❌ HALT: Agent Mail unavailable - cannot orchestrate

Parallel execution requires Agent Mail for:
- Worker registration and identity
- File reservation to prevent conflicts
- Cross-track dependency notifications
- Progress monitoring and blocker resolution

Options:
1. Wait for Agent Mail to become available
2. Run sequential: /conductor-implement (no parallel workers)
```

**Worker:**
```text
❌ HALT: Cannot initialize session - Agent Mail unavailable

Worker cannot proceed without:
- File reservations (risk of conflicts)
- Message capability (cannot report progress/blockers)

Returning control to orchestrator.
```

## No Sequential Fallback

Previous behavior (REMOVED):
```python
# ❌ DO NOT DO THIS
if not AGENT_MAIL_AVAILABLE:
    return conductor_implement(track_id)  # Silent degradation
```

Current behavior (REQUIRED):
```python
# ✅ HALT explicitly
if not AGENT_MAIL_AVAILABLE:
    print("❌ HALT: Agent Mail required for orchestration")
    return {"status": "HALTED", "reason": "Agent Mail unavailable"}
```

## Partial Failures (Mid-Orchestration)

If Agent Mail becomes unavailable after workers are spawned:

1. **Active workers continue current bead** - Finish what's in progress
2. **Workers HALT after current bead** - Cannot claim next safely
3. **Orchestrator logs and waits** - Retry Agent Mail periodically
4. **Recovery when available** - Resume monitoring, workers re-register

```python
# Mid-orchestration failure handling
while not all_complete:
    try:
        status = fetch_inbox(project_key=PROJECT_PATH, agent_name=ORCHESTRATOR)
        # Normal monitoring continues
    except McpUnavailable:
        print("⚠️ Agent Mail unavailable - waiting for recovery")
        sleep(60)
        continue  # Retry, don't fall back
```

## Recovery

If Agent Mail becomes available after failure:

1. Orchestrator re-establishes connection via `health_check`
2. Workers can re-register and continue
3. Resume normal monitoring loop
4. Check `bd status` to reconcile any missed updates

## Why No Fallback?

| Risk | Why HALT is correct |
|------|---------------------|
| File conflicts | Parallel workers WILL conflict without reservations |
| Lost progress | No way to detect worker completion/failure |
| Stale workers | Cannot detect or recover stale workers |
| Silent failures | Users expect parallel execution, not degraded mode |
| Data corruption | Concurrent edits without coordination = corruption |

## Implementation Notes

- Health check before any orchestration: `health_check(reason="...")`
- Timeout on MCP calls: ~3 seconds
- Retry Agent Mail 3 times with exponential backoff before HALT
- Log all failures with timestamps for debugging
- Return structured error for user decision
