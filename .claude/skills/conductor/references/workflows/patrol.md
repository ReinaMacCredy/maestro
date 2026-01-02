# Patrol Workflow

## Purpose

Scan for and resolve issues with stale beads, orphaned tasks, and abandoned sessions during orchestrated execution.

## Prerequisites

- Conductor environment initialized
- Beads CLI (`bd`) available
- At least one track with beads exists

## When to Use

| Trigger | Description |
|---------|-------------|
| `/conductor-patrol` | Manual patrol scan |
| Orchestrator Phase 5 | Automatic during monitoring |
| Session recovery | After crash or disconnect |
| End of day | Cleanup before signing off |

## Workflow Steps

### Phase 1: Session Scan

1. **List Session Locks**
   ```bash
   ls -la .conductor/session-lock_*.json 2>/dev/null
   ```

2. **Check Each Session**
   - Read lock file metadata
   - Parse `last_heartbeat` timestamp
   - Determine session state:
   
   | Last Heartbeat | State |
   |----------------|-------|
   | < 10 minutes | ACTIVE |
   | 10-30 minutes | STALE |
   | > 30 minutes | ABANDONED |

3. **Check Agent Mail Status**
   ```python
   for session in sessions:
       try:
           whois(agent_name=session.agent)
           session.agent_status = "REGISTERED"
       except:
           session.agent_status = "UNKNOWN"
   ```

### Phase 2: Stale Bead Detection

1. **Query Stale Beads**
   ```bash
   bd list --stale=30m --status in_progress --json
   ```

2. **Categorize by Duration**
   
   | Duration | Severity | Recommended Action |
   |----------|----------|-------------------|
   | 30-60m | ⚠️ Warning | Ping worker |
   | 60-120m | ❌ Critical | Consider takeover |
   | > 2h | 💀 Abandoned | Takeover or cleanup |

3. **Check Heartbeat Status**
   ```bash
   bd list --heartbeat-stale --json
   ```
   
   Beads without recent heartbeat are more likely abandoned.

### Phase 3: Orphaned Task Detection

1. **Find Tasks Without Assignee**
   ```bash
   bd list --status in_progress --json | \
     jq '.[] | select(.assignee == null or .assignee == "")'
   ```

2. **Find Tasks With Unknown Assignee**
   ```python
   for bead in in_progress_beads:
       if bead.assignee and not agent_exists(bead.assignee):
           orphans.append(bead)
   ```

### Phase 4: Blocked Task Resolution

1. **Find Blocked Tasks**
   ```bash
   bd list --status blocked --json
   ```

2. **Check Dependency Status**
   ```python
   for bead in blocked_beads:
       deps = get_dependencies(bead)
       if all(dep.status == "closed" for dep in deps):
           unblocked_candidates.append(bead)
   ```

3. **Suggest Unblocking**
   ```bash
   bd update <bead-id> --status ready --notes "Dependencies resolved"
   ```

### Phase 5: Action Selection

Present findings and action menu:

```
┌─ PATROL SUMMARY ────────────────────────────────┐
│ Sessions: 2 active, 1 stale                     │
│ Stale beads: 3 (1 warning, 2 critical)          │
│ Orphaned tasks: 1                               │
│ Unblockable tasks: 2                            │
│                                                 │
│ Actions:                                        │
│ [T] Takeover stale/orphaned tasks               │
│ [C] Cleanup stale sessions                      │
│ [P] Ping stale workers                          │
│ [R] Reassign to available workers               │
│ [U] Unblock resolved tasks                      │
│ [S] Skip - no action                            │
└─────────────────────────────────────────────────┘
```

### Phase 6: Execute Actions

#### Takeover (T)

```bash
# For each target bead
bd update <bead-id> --assignee null --notes "Released by patrol: stale"
bd claim <bead-id>

# Transfer file reservations
force_release_file_reservation(
  file_reservation_id=<id>,
  note="Released by patrol for takeover"
)
file_reservation_paths(
  agent_name="CurrentAgent",
  paths=[<bead_file_scope>]
)
```

#### Cleanup (C)

```bash
# Remove stale session locks
rm .conductor/session-lock_*.json

# Archive pending operations
mkdir -p .conductor/archive
mv .conductor/pending_*.jsonl .conductor/archive/ 2>/dev/null
```

#### Ping (P)

```python
send_message(
    to=[stale_bead.assignee],
    subject="[PING] Status check",
    body_md=f"""
## Status Check Required

Task `{stale_bead.id}` has been in progress for {stale_bead.stale_duration}.

Please respond with current status:
- Still working? Reply with progress update
- Blocked? Reply with blocker details
- Done? Close the bead with `bd close {stale_bead.id}`
""",
    ack_required=True
)
```

#### Reassign (R)

```python
# Get available workers
available = get_available_workers()

# Present selection
for i, worker in enumerate(available):
    print(f"[{i+1}] {worker.name} ({worker.task_count} tasks)")

# Execute reassignment
bd_update(bead_id, assignee=selected_worker)
send_message(
    to=[selected_worker],
    subject="[ASSIGN] Task reassigned to you",
    body_md=f"Task {bead_id} has been reassigned to you by patrol."
)
```

#### Unblock (U)

```bash
# For each unblockable bead
bd update <bead-id> --status ready --notes "Unblocked by patrol: dependencies resolved"

# Notify waiting workers
send_message(
    to=["ALL"],
    subject="[DEP] Tasks unblocked",
    body_md="The following tasks are now ready: ..."
)
```

## Output Artifacts

```
.conductor/
├── patrol_log_<timestamp>.json   # Patrol run history
├── session-lock_*.json           # May be cleaned up
└── archive/                      # Archived pending ops
    └── pending_updates_*.jsonl
```

## Error Handling

| Error | Action |
|-------|--------|
| bd unavailable | HALT with install message |
| Agent Mail unavailable | DEGRADE - skip ping/reassign |
| No stale beads | Report clean state |
| Takeover fails | Log and skip to next |

## Integration

### With Orchestrator

Orchestrator triggers patrol during Phase 5 (Monitor):

```python
# Every 5 minutes during orchestration
def monitoring_loop():
    while not all_tracks_complete():
        patrol_scan()
        sleep(5 * 60)
```

### With Heartbeat

Patrol uses heartbeat data for better stale detection:

```bash
# Prefer heartbeat-stale over updated_at
bd list --heartbeat-stale --json
```

### With Wisps

Patrol creates wisps for its own tracking:

```bash
bd create "Patrol run ${timestamp}" --wisp
# ... run checks ...
bd burn wisp-123  # Or squash if issues found
```

## Related

- [../../../orchestrator/references/witness-patrol.md](../../../orchestrator/references/witness-patrol.md) - Witness patrol system
- [../../../orchestrator/references/heartbeat.md](../../../orchestrator/references/heartbeat.md) - Heartbeat protocol
- [../../../orchestrator/references/wisps.md](../../../orchestrator/references/wisps.md) - Ephemeral beads
- [../beads-session.md](../beads-session.md) - Session management
