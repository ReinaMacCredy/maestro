# Witness Patrol

> **Purpose**: Document the witness patrol system for monitoring worker health and task progress in multi-agent orchestration.

## Overview

Witness patrol is a background monitoring system that detects stuck workers, unblocked dependencies, load imbalances, and orphaned tasks. It runs periodically during orchestration to ensure work continues flowing.

## 4-Check Patrol Cycle

Each patrol cycle executes four checks in sequence:

```
┌─────────────────────────────────────────────────────────────┐
│                     PATROL CYCLE                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   CHECK 1: STALE TASKS                                      │
│   ├─ Query: bd list --stale=30m --status in_progress        │
│   └─ Action: Ping worker or escalate                        │
│                                                             │
│   CHECK 2: UNBLOCKED TASKS                                  │
│   ├─ Query: bd ready --json (compare to previous)           │
│   └─ Action: Notify waiting workers                         │
│                                                             │
│   CHECK 3: LOAD BALANCE                                     │
│   ├─ Query: bd list --json | group by assignee              │
│   └─ Action: Redistribute if imbalance > 2                  │
│                                                             │
│   CHECK 4: ORPHANED TASKS                                   │
│   ├─ Query: bd list --status in_progress --assignee null    │
│   └─ Action: Assign to available worker                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Check 1: Stale Task Detection

Identifies tasks that have been `in_progress` for too long without updates.

```bash
# Query for stale tasks (> 30 minutes)
bd list --stale=30m --status in_progress --json
```

**Thresholds:**

| Duration | Severity | Action |
|----------|----------|--------|
| 30-60 min | Warning | Send ping message to worker |
| 60-120 min | High | Escalate to orchestrator |
| > 2 hours | Critical | Force reassign task |

**Response Protocol:**

```python
def handle_stale(bead):
    wisp = bd_create(f"Check stale: {bead.id}", wisp=True)
    
    # Try pinging worker
    send_message(
        to=[bead.assignee],
        subject="[PING] Status check",
        body=f"Task {bead.id} appears stale. Please respond."
    )
    
    # Wait for pong (with timeout)
    if await_pong(bead.assignee, timeout=5m):
        bd_burn(wisp)  # Worker alive, no action
    else:
        # Escalate
        bd_squash(wisp, title=f"STALE: {bead.id} - worker unresponsive")
        escalate_to_orchestrator(bead)
```

### Check 2: Unblocked Task Detection

Detects when blocked tasks become unblocked due to dependency completion.

```bash
# Get currently ready tasks
bd ready --json > /tmp/ready_now.json

# Compare to previous snapshot
diff /tmp/ready_prev.json /tmp/ready_now.json
```

**Notification Protocol:**

```python
def handle_unblocked(bead):
    # Find workers waiting on this dependency
    waiters = find_waiters_for(bead.id)
    
    for waiter in waiters:
        send_message(
            to=[waiter],
            subject="[DEP] Dependency completed",
            body=f"Task {bead.id} is now ready. You can proceed."
        )
```

### Check 3: Load Balance Check

Detects imbalances in task distribution across workers.

```bash
# Get task counts per assignee
bd list --status in_progress --json | \
  jq 'group_by(.assignee) | map({assignee: .[0].assignee, count: length})'
```

**Imbalance Threshold:** Redistribute when any worker has 2+ more tasks than another.

```python
def check_load_balance(workers):
    counts = {w: count_tasks(w) for w in workers}
    max_count = max(counts.values())
    min_count = min(counts.values())
    
    if max_count - min_count > 2:
        # Redistribution needed
        overloaded = [w for w, c in counts.items() if c == max_count]
        underloaded = [w for w, c in counts.items() if c == min_count]
        
        # Move one task from overloaded to underloaded
        redistribute(from_worker=overloaded[0], to_worker=underloaded[0])
```

### Check 4: Orphaned Task Detection

Finds tasks marked `in_progress` but with no assignee.

```bash
# Find orphaned tasks
bd list --status in_progress --json | jq '.[] | select(.assignee == null)'
```

**Recovery Protocol:**

```python
def handle_orphaned(bead):
    # Find available worker
    available = get_available_workers()
    
    if available:
        # Assign to first available
        bd_update(bead.id, assignee=available[0])
        send_message(
            to=[available[0]],
            subject="[ASSIGN] Orphaned task assigned",
            body=f"Task {bead.id} was orphaned. You are now assigned."
        )
    else:
        # No workers available, mark as blocked
        bd_update(bead.id, status="blocked", 
                  notes="No workers available for orphaned task")
```

## Backoff Strategy

Patrol adapts its frequency based on activity level:

```
┌─────────────────────────────────────────────────────────────┐
│                   BACKOFF STRATEGY                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Activity Level    Patrol Interval    Checks               │
│   ──────────────    ───────────────    ──────               │
│   HIGH (active)     2 minutes          All 4 checks         │
│   NORMAL            5 minutes          All 4 checks         │
│   LOW (quiet)       10 minutes         Stale + orphan only  │
│   IDLE              15 minutes         Stale only           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Activity Detection

```python
def detect_activity_level():
    # Count messages in last 5 minutes
    recent_messages = count_messages(since="5m")
    
    # Count bead updates in last 5 minutes
    recent_updates = count_bead_updates(since="5m")
    
    activity = recent_messages + recent_updates
    
    if activity > 10:
        return "HIGH"
    elif activity > 3:
        return "NORMAL"
    elif activity > 0:
        return "LOW"
    else:
        return "IDLE"
```

## Wisp Usage for Patrol

Patrol uses wisps (ephemeral beads) to track its own activities without cluttering the permanent issue history.

### Patrol Wisp Pattern

```python
def patrol_cycle():
    # Create wisp for this patrol run
    wisp = bd_create(f"Patrol {timestamp()}", wisp=True)
    
    findings = []
    
    # Run all checks
    findings.extend(check_stale_tasks())
    findings.extend(check_unblocked_tasks())
    findings.extend(check_load_balance())
    findings.extend(check_orphaned_tasks())
    
    if findings:
        # Issues found - squash wisp to permanent bead
        bd_squash(wisp.id, 
            title=f"PATROL: Found {len(findings)} issues",
            notes=format_findings(findings))
    else:
        # Clean run - burn the wisp
        bd_burn(wisp.id)
```

### Wisp Commands Used

```bash
# Create patrol wisp
bd create "Patrol cycle 2026-01-03T12:00:00" --wisp

# On clean patrol
bd burn wisp-123

# On issues found
bd squash wisp-123 --title "PATROL: 2 stale workers found"
```

## Reassignment Protocol

When a task needs to be reassigned (due to stale worker or orphan):

```
┌─────────────────────────────────────────────────────────────┐
│                 REASSIGNMENT PROTOCOL                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   1. VALIDATE                                               │
│      ├─ Confirm task is actually stale/orphaned             │
│      └─ Check current assignee status                       │
│                                                             │
│   2. RELEASE                                                │
│      ├─ Clear current assignee                              │
│      └─ Release any file reservations                       │
│                                                             │
│   3. NOTIFY                                                 │
│      ├─ Send message to original assignee (if any)          │
│      └─ Log reassignment reason                             │
│                                                             │
│   4. CLAIM                                                  │
│      ├─ Use atomic claim for new worker                     │
│      └─ Reserve files for new worker                        │
│                                                             │
│   5. HANDOFF                                                │
│      ├─ Share context from original worker                  │
│      └─ Point to relevant commits/files                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Atomic Reassignment

```bash
# Step 1: Release from original
bd update <bead-id> --assignee null --notes "Released by patrol: stale"

# Step 2: Claim for new worker (atomic)
bd claim <bead-id> --agent NewWorker

# Step 3: File reservation transfer
release_file_reservations(agent="OldWorker", paths=[...])
file_reservation_paths(agent="NewWorker", paths=[...])
```

### Handoff Message

```python
def send_handoff_message(old_worker, new_worker, bead):
    send_message(
        to=[new_worker],
        cc=[old_worker],  # FYI to original worker
        subject=f"[HANDOFF] Task {bead.id} reassigned",
        body=f"""
## Task Reassigned

**From:** {old_worker or "Unassigned"}
**To:** {new_worker}
**Reason:** {bead.reassignment_reason}

## Context

{bead.notes}

## Files to Review

{format_file_list(bead.file_scope)}
"""
    )
```

## Configuration

### Patrol Settings

```yaml
# conductor/patrol.yaml (or in workflow.md)
patrol:
  enabled: true
  base_interval: 5m
  
  stale:
    warning_threshold: 30m
    critical_threshold: 2h
    auto_reassign: true
    
  load_balance:
    imbalance_threshold: 2
    auto_redistribute: false  # Requires confirmation
    
  orphan:
    auto_assign: true
    
  backoff:
    high_activity: 2m
    normal_activity: 5m
    low_activity: 10m
    idle: 15m
```

## Integration Points

### With Orchestrator

```python
# Orchestrator starts patrol on Phase 5 (Monitor)
def start_monitoring():
    patrol = WitnessPatrol(
        epic_id=current_epic,
        workers=registered_workers
    )
    patrol.start()
```

### With Agent Mail

```python
# Patrol uses Agent Mail for all notifications
def notify_worker(worker, message):
    send_message(
        project_key=PROJECT_KEY,
        sender_name="WitnessPatrol",
        to=[worker],
        subject=message.subject,
        body_md=message.body
    )
```

### With Heartbeat

Patrol relies on heartbeat system for worker liveness detection:

```python
def is_worker_alive(worker):
    # Check last heartbeat
    beads = bd_list(assignee=worker, json=True)
    for bead in beads:
        if bead.last_heartbeat:
            age = now() - bead.last_heartbeat
            if age < 10m:  # Stale threshold
                return True
    return False
```

## Related

- [heartbeat.md](heartbeat.md) - Worker heartbeat protocol
- [wisps.md](wisps.md) - Ephemeral beads for patrol tracking
- [beads-stale.md](beads-stale.md) - Stale detection query interface
- [beads-atomic-claim.md](beads-atomic-claim.md) - Safe reassignment
- [worker-protocol-v2.md](worker-protocol-v2.md) - Worker liveness expectations
