# Beads Stale Detection

> **Purpose**: Document stale detection for identifying abandoned or stuck work items.

## Overview

Stale detection enables the orchestrator and witness patrol to identify beads that have been in progress too long without updates, indicating potential worker failures or abandoned tasks.

## Query Interface

### CLI Syntax

```bash
# List beads stale for more than 30 minutes
bd list --stale=30m

# List beads stale for more than 1 hour
bd list --stale=1h

# Combine with status filter
bd list --stale=30m --status in_progress

# Combine with assignee filter
bd list --stale=1h --assignee PinkHill --json
```

### Duration Formats

| Format | Example | Meaning |
|--------|---------|---------|
| Minutes | `30m` | 30 minutes |
| Hours | `2h` | 2 hours |
| Days | `1d` | 1 day |
| Combined | `1h30m` | 1 hour 30 minutes |

## Stale Calculation

### Formula

```
stale = now() - updated_at > duration
```

### Example

```
Bead updated_at: 2026-01-03T01:30:00Z
Current time:    2026-01-03T02:15:00Z
Duration since:  45 minutes

Query: bd list --stale=30m
Result: Bead IS stale (45m > 30m)

Query: bd list --stale=1h
Result: Bead is NOT stale (45m < 60m)
```

### Fields Used

| Field | Role |
|-------|------|
| `updated_at` | Timestamp of last modification |
| `status` | Only `in_progress` beads are considered stale |

## Use Cases

### 1. Orchestrator Health Check

```bash
# Check for stuck workers during monitoring phase
bd list --stale=30m --status in_progress --json
```

If results found, orchestrator may:
- Send ping message to assigned worker
- Escalate to witness patrol
- Reassign bead to backup worker

### 2. Witness Patrol Sweep

```bash
# Periodic sweep for abandoned work
bd list --stale=1h --json | jq '.[] | {id, assignee, updated_at}'
```

Actions:
- Log warning for each stale bead
- Check agent liveness via Agent Mail
- Trigger recovery protocol if agent unresponsive

### 3. Session Cleanup

```bash
# Find beads from crashed sessions (very stale)
bd list --stale=24h --status in_progress
```

## Integration with Witness Patrol

### Detection Flow

```
┌─────────────────┐
│ Witness Patrol  │
│  (every 5min)   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ bd list         │
│ --stale=30m     │
│ --json          │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│ Stale found?    │────▶│ Check agent     │
│                 │ yes │ liveness        │
└────────┬────────┘     └────────┬────────┘
         │ no                    │
         ▼                       ▼
      (continue)        ┌─────────────────┐
                        │ Agent alive?    │
                        └────────┬────────┘
                                 │
                    ┌────────────┴────────────┐
                    ▼ yes                     ▼ no
            ┌─────────────┐           ┌─────────────┐
            │ Send ping   │           │ Reassign    │
            │ message     │           │ bead        │
            └─────────────┘           └─────────────┘
```

### Witness Patrol Configuration

```yaml
# Example patrol configuration
patrol:
  interval: 5m
  stale_threshold: 30m
  escalation_threshold: 1h
  actions:
    - ping_agent
    - notify_orchestrator
    - force_reassign
```

## JSON Output Schema

```json
[
  {
    "id": "my-workflow:3-zyci.3.2",
    "title": "Document stale detection",
    "status": "in_progress",
    "assignee": "PinkHill",
    "updated_at": "2026-01-03T01:30:00Z",
    "stale_duration": "45m"
  }
]
```

## Related

- [beads-assignee.md](beads-assignee.md) - Assignee tracking for stale owner identification
- [beads-atomic-claim.md](beads-atomic-claim.md) - Safe reassignment after stale detection
- [monitoring.md](monitoring.md) - Orchestrator monitoring patterns
