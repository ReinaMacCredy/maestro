# Heartbeat Protocol

> **Purpose**: Document the heartbeat system for worker liveness detection during multi-agent orchestration.

## Overview

Heartbeats are periodic status signals from workers indicating they are still active. The orchestrator and witness patrol use heartbeats to detect stuck or crashed workers.

## Beads Integration

### --heartbeat Flag

Workers send heartbeats via the `bd` CLI:

```bash
# Send heartbeat for current task
bd heartbeat <bead-id>

# Send heartbeat with status message
bd heartbeat <bead-id> --message "Working on tests"
```

### last_heartbeat Field

Each bead tracks the last heartbeat timestamp:

```yaml
id: my-workflow:3-zyci.2.1
title: "Implement login endpoint"
status: in_progress
assignee: PinkHill
last_heartbeat: "2026-01-03T12:15:00Z"
updated_at: "2026-01-03T12:00:00Z"
```

**Note:** `last_heartbeat` is distinct from `updated_at`:
- `updated_at` changes on any field modification
- `last_heartbeat` only changes on explicit heartbeat

### --heartbeat-stale Query

Find beads with stale heartbeats:

```bash
# Beads with no heartbeat in 10 minutes
bd list --heartbeat-stale --json

# Combine with status filter
bd list --heartbeat-stale --status in_progress --json

# Custom threshold
bd list --heartbeat-stale=15m --json
```

## Worker Heartbeat Protocol

### Interval

Workers MUST send heartbeats at regular intervals:

| Setting | Value | Notes |
|---------|-------|-------|
| Interval | 5 minutes | Default heartbeat frequency |
| Tolerance | ±1 minute | Acceptable jitter |
| Stale threshold | 10 minutes | No heartbeat = potentially dead |

### Implementation Pattern

```python
class WorkerHeartbeat:
    def __init__(self, bead_id: str, interval: int = 300):
        self.bead_id = bead_id
        self.interval = interval  # 5 minutes in seconds
        self.running = False
        
    async def start(self):
        self.running = True
        while self.running:
            await self.send_heartbeat()
            await asyncio.sleep(self.interval)
    
    async def send_heartbeat(self):
        # Send heartbeat via bd CLI
        subprocess.run(["bd", "heartbeat", self.bead_id])
        
    def stop(self):
        self.running = False
```

### Worker Lifecycle

```
┌─────────────────────────────────────────────────────────────┐
│                  WORKER HEARTBEAT LIFECYCLE                 │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   CLAIM TASK                                                │
│        │                                                    │
│        ▼                                                    │
│   ┌──────────────────┐                                      │
│   │ Start heartbeat  │                                      │
│   │ timer (5min)     │                                      │
│   └────────┬─────────┘                                      │
│            │                                                │
│            ▼                                                │
│   ┌──────────────────────────────────────────┐              │
│   │              WORK LOOP                    │              │
│   │  ┌─────────────────────────────────────┐ │              │
│   │  │ Every 5 minutes:                    │ │              │
│   │  │   bd heartbeat <bead-id>            │ │              │
│   │  └─────────────────────────────────────┘ │              │
│   └────────┬─────────────────────────────────┘              │
│            │                                                │
│            ▼                                                │
│   ┌──────────────────┐                                      │
│   │ Task complete    │                                      │
│   │ Stop heartbeat   │                                      │
│   │ bd close <id>    │                                      │
│   └──────────────────┘                                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Stale Threshold

### Detection Logic

```python
def is_heartbeat_stale(bead, threshold_minutes=10):
    if not bead.last_heartbeat:
        # Never sent heartbeat - use updated_at as fallback
        return (now() - bead.updated_at).minutes > threshold_minutes
    
    return (now() - bead.last_heartbeat).minutes > threshold_minutes
```

### Threshold Configuration

| Context | Threshold | Rationale |
|---------|-----------|-----------|
| Active orchestration | 10 minutes | Quick detection |
| Overnight/background | 30 minutes | Allow for breaks |
| Critical tasks | 5 minutes | High urgency |

## Integration Points

### With Witness Patrol

Patrol uses heartbeat-stale for more accurate detection:

```bash
# Patrol check (every 5 minutes)
stale_beads=$(bd list --heartbeat-stale --status in_progress --json)

for bead in $stale_beads; do
    # Check if worker is still registered
    if agent_exists(bead.assignee); then
        send_ping(bead.assignee)
    else
        mark_for_reassignment(bead)
    fi
done
```

### With Orchestrator

Orchestrator monitors heartbeats during Phase 5:

```python
def monitor_workers():
    while not all_complete():
        # Check for stale heartbeats
        stale = bd_list(heartbeat_stale=True, json=True)
        
        for bead in stale:
            log_warning(f"Stale heartbeat: {bead.id} ({bead.assignee})")
            
            if bead.stale_duration > 30:
                escalate_to_patrol(bead)
        
        sleep(5 * 60)  # Check every 5 minutes
```

### With Agent Mail

Heartbeat can trigger Agent Mail messages:

```python
# On heartbeat, optionally notify orchestrator
bd heartbeat <bead-id> --notify

# Generates message:
send_message(
    to=["Orchestrator"],
    subject="[HEARTBEAT] Worker alive",
    body=f"Worker {agent_name} heartbeat for {bead_id}"
)
```

## CLI Reference

### Commands

| Command | Description |
|---------|-------------|
| `bd heartbeat <id>` | Send heartbeat for bead |
| `bd heartbeat <id> --message "..."` | Heartbeat with status |
| `bd heartbeat <id> --notify` | Heartbeat + notify orchestrator |

### Query Flags

| Flag | Description |
|------|-------------|
| `--heartbeat-stale` | Filter beads with stale heartbeat (>10m default) |
| `--heartbeat-stale=<duration>` | Custom stale threshold |

### Examples

```bash
# Worker sends heartbeat during work
bd heartbeat my-workflow:3-zyci.2.1

# Worker sends heartbeat with progress
bd heartbeat my-workflow:3-zyci.2.1 --message "Tests passing, refactoring"

# Patrol queries for stale workers
bd list --heartbeat-stale --status in_progress --json

# Check for very stale (>30 min)
bd list --heartbeat-stale=30m --json
```

## Bead Schema Extension

```yaml
# Extended bead schema with heartbeat
id: string
title: string
status: string
assignee: string | null
updated_at: datetime
last_heartbeat: datetime | null  # NEW FIELD
heartbeat_message: string | null # Optional status from last heartbeat
```

## Error Handling

### Missed Heartbeats

| Missed Count | Action |
|--------------|--------|
| 1 (5-10 min) | Warning in patrol log |
| 2 (10-15 min) | Ping worker via Agent Mail |
| 3+ (>15 min) | Escalate, consider reassignment |

### Heartbeat Failures

```python
def send_heartbeat_safe(bead_id):
    try:
        subprocess.run(["bd", "heartbeat", bead_id], check=True)
    except subprocess.CalledProcessError:
        # Log but don't crash worker
        log_warning(f"Heartbeat failed for {bead_id}")
        # Retry on next interval
```

## Best Practices

1. **Start heartbeat immediately** after claiming a task
2. **Stop heartbeat** before closing the task
3. **Include progress messages** for visibility
4. **Handle failures gracefully** - don't crash on heartbeat error
5. **Use notify flag sparingly** - avoid message spam

## Related

- [witness-patrol.md](witness-patrol.md) - Uses heartbeat for detection
- [beads-stale.md](beads-stale.md) - General stale detection
- [worker-protocol-v2.md](worker-protocol-v2.md) - Worker lifecycle
- [wisps.md](wisps.md) - Patrol tracking with ephemeral beads
