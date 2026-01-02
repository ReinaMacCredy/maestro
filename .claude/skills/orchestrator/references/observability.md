# Observability

Metrics, state tracking, and logging for orchestrator visibility.

## Metrics to Track

### Core Metrics

| Metric | Description | Source |
|--------|-------------|--------|
| `beads_closed` | Total beads completed | `bd list --status=closed --json` |
| `beads_open` | Currently open beads | `bd list --status=open --json` |
| `beads_blocked` | Beads waiting on dependencies | `bd list --status=blocked --json` |
| `blockers_count` | Active cross-track blockers | Agent Mail `[BLOCKER]` messages |
| `retries` | Task retry attempts | `implement_state.json` |
| `worker_count` | Active workers | Agent Mail registrations |
| `wave_count` | Execution waves | `implement_state.json` |

### Per-Worker Metrics

| Metric | Description |
|--------|-------------|
| `tasks_assigned` | Initial task count |
| `tasks_completed` | Closed beads |
| `tasks_stolen` | Received via work stealing |
| `tasks_donated` | Given away via work stealing |
| `duration_seconds` | Time from start to summary |
| `heartbeat_count` | Heartbeats sent |

### Timing Metrics

| Metric | Description |
|--------|-------------|
| `epic_duration` | Total time from spawn to completion |
| `avg_task_duration` | Mean time per bead |
| `max_task_duration` | Longest single bead |
| `idle_time` | Time workers spent waiting |
| `coordination_overhead` | Time spent on Agent Mail ops |

## State File: implement_state.json

Orchestrator maintains state in `conductor/tracks/<id>/implement_state.json`:

```json
{
  "version": "2.0",
  "epic_id": "my-workflow:3-zyci",
  "execution_mode": "PARALLEL_DISPATCH",
  "orchestrator_name": "PurpleMountain",
  "started_at": "2025-12-30T01:30:00Z",
  "last_poll": "2025-12-30T02:15:00Z",
  
  "workers": {
    "BlueLake": {
      "track": "A",
      "status": "complete",
      "beads_assigned": 6,
      "beads_closed": 6,
      "beads_stolen": 0,
      "beads_donated": 1,
      "summary_received": true,
      "started_at": "2025-12-30T01:30:05Z",
      "completed_at": "2025-12-30T02:00:00Z",
      "last_heartbeat": "2025-12-30T01:55:00Z"
    },
    "GreenCastle": {
      "track": "B",
      "status": "in_progress",
      "beads_assigned": 5,
      "beads_closed": 4,
      "current_bead": "my-workflow:3-zyci.5.1",
      "summary_received": false,
      "started_at": "2025-12-30T01:30:05Z",
      "last_heartbeat": "2025-12-30T02:12:00Z"
    }
  },
  
  "waves": [
    {
      "wave": 1,
      "beads": ["zyci.1.1", "zyci.2.1", "zyci.3.1"],
      "started_at": "2025-12-30T01:30:00Z",
      "completed_at": "2025-12-30T01:45:00Z",
      "status": "complete"
    },
    {
      "wave": 2,
      "beads": ["zyci.4.1", "zyci.5.1"],
      "started_at": "2025-12-30T01:45:00Z",
      "status": "in_progress"
    }
  ],
  
  "metrics": {
    "beads_total": 26,
    "beads_closed": 22,
    "beads_blocked": 1,
    "blockers_resolved": 3,
    "retries": 2,
    "work_steals": 1
  },
  
  "expected_workers": ["BlueLake", "GreenCastle", "RedStone"],
  "workers_with_summaries": ["BlueLake"]
}
```

### State File Updates

Update state file during key events:

```python
# On worker spawn
state["workers"][worker_name] = {
    "track": track_id,
    "status": "spawned",
    "started_at": now(),
    "beads_assigned": len(track.beads)
}
save_state()

# On heartbeat received
state["workers"][worker_name]["last_heartbeat"] = now()
state["last_poll"] = now()
save_state()

# On bead closed
state["workers"][worker_name]["beads_closed"] += 1
state["metrics"]["beads_closed"] += 1
save_state()

# On summary received
state["workers"][worker_name]["summary_received"] = True
state["workers"][worker_name]["status"] = "complete"
state["workers"][worker_name]["completed_at"] = now()
state["workers_with_summaries"].append(worker_name)
save_state()
```

## Patrol Log Format

Orchestrator writes patrol entries during Phase 5 monitoring:

```
# patrol.log
[2025-12-30T01:30:00Z] PATROL: Starting monitor loop for epic my-workflow:3-zyci
[2025-12-30T01:30:30Z] PATROL: Workers spawned: BlueLake, GreenCastle, RedStone
[2025-12-30T01:31:00Z] PATROL: BlueLake claimed zyci.1.1
[2025-12-30T01:31:30Z] PATROL: Poll #1 - 0/26 beads closed
[2025-12-30T01:32:00Z] PATROL: GreenCastle heartbeat received
[2025-12-30T01:35:00Z] PATROL: BlueLake closed zyci.1.1
[2025-12-30T01:35:00Z] PATROL: Poll #2 - 1/26 beads closed
[2025-12-30T01:40:00Z] PATROL: BLOCKER detected - zyci.3.2 blocked by zyci.1.3
[2025-12-30T01:42:00Z] PATROL: Blocker resolved - zyci.1.3 complete
[2025-12-30T01:50:00Z] PATROL: IMBALANCE detected - RedStone: 5 pending, BlueLake: 0 pending
[2025-12-30T01:50:30Z] PATROL: Work steal - zyci.6.1 from RedStone to BlueLake
[2025-12-30T02:00:00Z] PATROL: BlueLake summary received
[2025-12-30T02:15:00Z] PATROL: All workers complete - 26/26 beads closed
[2025-12-30T02:15:00Z] PATROL: Epic my-workflow:3-zyci complete
```

### Patrol Log Levels

| Level | Format | Use |
|-------|--------|-----|
| INFO | `PATROL: <message>` | Normal operations |
| WARN | `PATROL: ⚠️ <message>` | Potential issues |
| ERROR | `PATROL: ❌ <message>` | Failures requiring action |
| BLOCKER | `PATROL: BLOCKER <message>` | Cross-track dependencies |
| IMBALANCE | `PATROL: IMBALANCE <message>` | Load imbalance detected |

## Summary Generation

At epic completion, generate comprehensive summary:

```python
def generate_summary(state):
    duration = state["completed_at"] - state["started_at"]
    
    summary = f"""
## Epic Summary: {state["epic_id"]}

### Overview
- **Duration**: {format_duration(duration)}
- **Workers**: {len(state["expected_workers"])} spawned, {len(state["workers_with_summaries"])} completed
- **Beads**: {state["metrics"]["beads_closed"]}/{state["metrics"]["beads_total"]} closed
- **Waves**: {len(state["waves"])}

### Metrics
| Metric | Value |
|--------|-------|
| Blockers resolved | {state["metrics"]["blockers_resolved"]} |
| Retries | {state["metrics"]["retries"]} |
| Work steals | {state["metrics"]["work_steals"]} |

### Per-Worker Summary
"""
    
    for name, worker in state["workers"].items():
        worker_duration = worker.get("completed_at", now()) - worker["started_at"]
        summary += f"""
#### {name} (Track {worker["track"]})
- Status: {worker["status"]}
- Beads: {worker["beads_closed"]}/{worker["beads_assigned"]}
- Duration: {format_duration(worker_duration)}
- Summary received: {"✓" if worker["summary_received"] else "✗"}
"""
    
    return summary
```

### Summary Message

```python
send_message(
  project_key=PROJECT_KEY,
  sender_name=ORCHESTRATOR_NAME,
  to=expected_workers,
  thread_id=epic_id,
  subject=f"[SUMMARY] Epic {epic_id} Complete",
  body_md=generate_summary(state)
)
```

## Dashboard View

For real-time monitoring, orchestrator can display:

```
┌─────────────────────────────────────────────────────────────┐
│ Epic: my-workflow:3-zyci          Duration: 0h 45m 00s      │
├─────────────────────────────────────────────────────────────┤
│ Progress: ████████████████████░░░░ 22/26 (85%)              │
│                                                             │
│ Workers:                                                    │
│   BlueLake    [████████████] 6/6  ✓ Complete               │
│   GreenCastle [████████░░░░] 4/5  ~ In Progress            │
│   RedStone    [████████████] 12/15 ~ In Progress           │
│                                                             │
│ Blockers: 0 active, 3 resolved                              │
│ Retries: 2                                                  │
│ Work Steals: 1                                              │
│                                                             │
│ Last Event: GreenCastle closed zyci.5.1 (2m ago)           │
└─────────────────────────────────────────────────────────────┘
```

## Related

- [workflow.md](workflow.md) - Phase 5 monitoring loop
- [preflight.md](preflight.md) - Session identity and detection
- [summary-protocol.md](summary-protocol.md) - Summary format standards
