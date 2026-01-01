# Monitoring Phase - Agent Mail Coordination

> **Track worker progress, detect issues, and resolve blockers via Agent Mail.**

## Overview

Orchestrator monitors workers through Agent Mail messages. Workers report:
- Progress updates
- Bead completions
- Blockers and questions
- Cross-track dependency completion

## Monitoring Loop

```python
while not all_complete:
    # 1. Check inbox for urgent messages
    urgent = fetch_inbox(
        project_key=project_path,
        agent_name=orchestrator_name,
        urgent_only=True,
        include_bodies=True
    )
    
    # 2. Handle blockers immediately
    for msg in urgent:
        handle_blocker(msg)
    
    # 3. Search for progress in epic thread
    progress = search_messages(
        project_key=project_path,
        query=f"thread:{epic_id} COMPLETE",
        limit=50
    )
    
    # 4. Check bead status
    status = bv_robot_triage(epic_id)
    
    # 5. Update state and log progress
    update_state(progress, status)
    
    # 6. Wait before next poll
    sleep(30)  # 30 second interval
```

## Message Types

### Progress Report

Workers send progress after each bead:

```markdown
Subject: [PROGRESS] Track 1: 4/6 beads complete

## Summary
- ✅ Completed: 1.1.1, 1.1.2, 1.1.3, 1.2.1
- 🔄 In Progress: 1.2.2
- ⏳ Remaining: 1.2.3

## Current Work
Creating SKILL.md with frontmatter...

## Files Changed
- skills/orchestrator/SKILL.md
- skills/orchestrator/references/workflow.md
```

### Bead Completion

Notification when bead closes:

```markdown
Subject: [COMPLETE] bd-102 - Create SKILL.md

✅ Bead bd-102 closed successfully.

## Changes
- Created skills/orchestrator/SKILL.md
- Added frontmatter with name, version, description

## Duration: 12 minutes
```

### Dependency Notification

When a blocking bead completes, notify waiting workers:

```markdown
Subject: [DEP] bd-102 COMPLETE - Track 2 unblocked

Bead bd-102 (Task 1.2.3) is complete. 
Track 2 can now proceed with bd-201 (Task 2.1.1).

## Files Available
- skills/orchestrator/SKILL.md (now exists)
```

### Blocker Report

Workers report when stuck:

```markdown
Subject: [BLOCKER] Track 2: File conflict on design/SKILL.md

## Problem
Cannot edit skills/design/SKILL.md - file is reserved by Track 1.

## Current Reservations
- skills/orchestrator/** (Track 1, BlueLake)
- skills/design/** (Track 2, GreenCastle) ← conflict

## Request
Please coordinate release of design files.
```

## Handling Blockers

### File Conflict

```python
# 1. Check reservations
reservations = list_reservations(project_key)

# 2. Identify holder
holder = find_reservation_holder(file_path, reservations)

# 3. Request release
send_message(
    to=[holder.agent_name],
    subject="File conflict resolution",
    body_md=f"Worker {requester} needs {file_path}. Can you release?",
    importance="high"
)
```

### Cross-Track Dependency Timeout

If waiting > 30 minutes:

```python
# 1. Check if blocking bead is still in progress
blocking_bead = bd_show(dep_bead_id)

if blocking_bead.status == "in_progress":
    # 2. Ping the worker
    send_message(
        to=[blocking_worker],
        subject=f"[PING] {dep_bead_id} status?",
        body_md="Dependency timeout (30 min). Status update needed.",
        importance="urgent"
    )
elif blocking_bead.status == "open":
    # 3. Bead not started - escalate
    log_warning(f"Blocking bead {dep_bead_id} never started")
```

### Worker Stale Detection

No heartbeat for 10 minutes:

```python
# 1. Check last activity
last_msg = search_messages(
    query=f"from:{worker_name}",
    limit=1
)
stale_minutes = minutes_since(last_msg.created_ts)

if stale_minutes > 10:
    # 2. Mark worker as stale
    state.workers[worker_name].status = "stale"
    
    # 3. Attempt to recover
    # Option A: Wait for natural recovery
    # Option B: Force release reservations and re-spawn
```

## Progress Tracking

### State File

Orchestrator maintains `implement_state.json`:

```json
{
  "execution_mode": "PARALLEL_DISPATCH",
  "orchestrator_name": "PurpleMountain",
  "epic_id": "my-workflow:3-3cmw",
  "workers": {
    "BlueLake": {
      "track": 1,
      "status": "complete",
      "beads_total": 6,
      "beads_closed": 6,
      "last_heartbeat": "2025-12-30T02:30:00Z"
    },
    "GreenCastle": {
      "track": 2,
      "status": "in_progress",
      "beads_total": 5,
      "beads_closed": 3,
      "current_bead": "my-workflow:3-3cmw.14",
      "last_heartbeat": "2025-12-30T02:45:00Z"
    }
  },
  "started_at": "2025-12-30T01:30:00Z",
  "last_poll": "2025-12-30T02:50:00Z"
}
```

### Progress Display

```text
📊 Epic Progress: 14/26 beads complete (54%)

Track Status:
├── Track 1 (BlueLake): 6/6 ✅ COMPLETE
├── Track 2 (GreenCastle): 3/5 🔄 in_progress
│   └── Current: my-workflow:3-3cmw.14 (routing.md)
└── Track 3 (RedStone): 5/15 🔄 in_progress
    └── Blocked by: my-workflow:3-3cmw.14

Estimated time remaining: 45 min
```

## Completion Detection

All workers complete when:

```python
def all_complete():
    # 1. Check worker state
    for worker in state.workers.values():
        if worker.status not in ["complete", "error"]:
            return False
    
    # 2. Verify via beads
    status = bv_robot_triage(epic_id)
    if status.open_count > 0:
        return False
    
    return True
```

## Agent Mail Tools Reference

| Tool | Purpose |
|------|---------|
| `fetch_inbox` | Get messages for orchestrator |
| `search_messages` | Query epic thread for keywords |
| `send_message` | Send to workers or all |
| `reply_message` | Reply to blocker reports |
| `mark_message_read` | Mark messages as processed |

## Timing Parameters

| Parameter | Value | Purpose |
|-----------|-------|---------|
| Poll interval | 30 sec | Time between inbox checks |
| Heartbeat | 5 min | Worker must send update |
| Stale threshold | 10 min | Mark worker as stale |
| Cross-dep timeout | 30 min | Escalate waiting dep |
