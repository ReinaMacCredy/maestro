# Coordination Modes

> **Note:** For autonomous execution (`ca`), see [autonomous.md](../../conductor/references/workflows/autonomous.md).
> Ralph is a third execution mode alongside `ci` (sequential) and `co` (parallel).

Orchestrator supports two modes based on task complexity and Agent Mail availability.

## Mode Comparison

| Mode | Agent Mail | Heartbeats | Use Case |
|------|------------|------------|----------|
| **Light** | Not required | No | Simple parallel tasks, no cross-deps, tasks <10 min |
| **Full** | Required | Yes (>10 min) | Complex coordination, blockers, cross-track deps |

## Mode Selection

```python
# Auto-select mode based on conditions
if not agent_mail_available():
    mode = "LIGHT"  # Fallback only when unavailable
else:
    mode = "FULL"   # Always use Agent Mail coordination when available
```

## Light Mode Behavior

- Workers execute via Task() and return structured results
- No Agent Mail registration, messaging, or heartbeats
- Orchestrator collects results from Task() return values
- Cross-track deps handled via Task() sequencing (spawn dependent tracks after blockers complete)

### Worker Protocol (3-Step)

```
┌─────────────────────────────────────────────────────────────┐
│  STEP 1: EXECUTE  (bd update/close - claim and work)       │
│  STEP 2: RETURN   (structured result via Task() return)    │
│  STEP 3: (none)   (no Agent Mail, no reservations)         │
└─────────────────────────────────────────────────────────────┘
```

**Light mode rules:**
- ❌ No Agent Mail registration or messaging
- ❌ No file reservations (rely on file scope isolation)
- ❌ No heartbeats
- ✅ Return structured summary via Task() return value

### Task Return Format

When Agent Mail unavailable, workers return structured results:

```python
return {
    "status": "SUCCEEDED",  # or "PARTIAL" or "FAILED"
    "files_changed": [
        {"path": "path/to/file.ts", "action": "added"},
        {"path": "path/to/other.ts", "action": "modified"}
    ],
    "key_decisions": [
        {"decision": "Used X pattern", "rationale": "because Y"}
    ],
    "issues": [],  # Empty if none
    "beads_closed": ["bd-101", "bd-102"]
}
```

Orchestrator collects these returns and aggregates into final summary.

## Full Mode Behavior

- Full Agent Mail protocol (register, message, heartbeat)
- Real-time progress monitoring via `agent-mail.js fetch-inbox`
- Cross-track dependency notifications
- Blocker resolution via `agent-mail.js reply-message`

### Worker Protocol (4-Step)

```
┌───────────────────────────────────────────────────────────────────────┐
│  STEP 1: INITIALIZE (agent-mail.js macro-start-session - FIRST)       │
│  STEP 2: EXECUTE    (bd update/close - claim and work)                │
│  STEP 3: REPORT     (agent-mail.js send-message - MANDATORY summary)  │
│  STEP 4: CLEANUP    (agent-mail.js release-file-reservations)         │
└───────────────────────────────────────────────────────────────────────┘
```

**Key rules:**
- ✅ STEP 1 must be FIRST action (orchestrator pre-registered you)
- ✅ STEP 3 must happen BEFORE returning (non-negotiable)
- ✅ Workers CAN use `bd update` and `bd close` directly
- ⏭️ Heartbeats only for tasks >10 minutes

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| max_workers | 3 | Maximum parallel workers |
| heartbeat_interval | 5 min | Worker heartbeat frequency |
| stale_threshold | 10 min | When to consider worker stale |
| cross_dep_timeout | 30 min | Max wait for cross-track dependency |

## Fallback Behavior

If Agent Mail unavailable:

```text
⚠️ Agent coordination unavailable - falling back to sequential execution
```

Routes to standard `/conductor-implement` instead.
