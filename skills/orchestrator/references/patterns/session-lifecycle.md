# Session Lifecycle Pattern

Worker session lifecycle with Agent Mail coordination.

## Overview

Each worker maintains its own session lifecycle:
1. Register with Agent Mail
2. Work on assigned beads
3. Report progress via messages
4. Complete and send summary

## Worker Session Start

```python
# 1. Register agent identity
register_agent(
  project_key=PROJECT_PATH,
  name=AGENT_NAME,  # e.g., "BlueLake"
  program="amp",
  model=MODEL,
  task_description=f"Worker for Track {TRACK_N}"
)

# 2. Check for cross-track dependency notifications
messages = fetch_inbox(
  project_key=PROJECT_PATH,
  agent_name=AGENT_NAME,
  include_bodies=True
)

# 3. Filter for dependency completions
for msg in messages:
    if "[DEP]" in msg.subject and "COMPLETE" in msg.subject:
        # Dependency satisfied, can proceed
        pass
```

## Worker Session Loop

```python
for bead_id in assigned_beads:
    # 1. Reserve files for this bead
    file_reservation_paths(
      project_key=PROJECT_PATH,
      agent_name=AGENT_NAME,
      paths=[FILE_SCOPE],
      ttl_seconds=3600
    )
    
    # 2. Claim bead
    bash(f"bd update {bead_id} --status in_progress")
    
    # 3. Do work
    implement_task(bead_id)
    
    # 4. Close bead
    bash(f"bd close {bead_id} --reason completed")
    
    # 5. Report completion
    send_message(
      project_key=PROJECT_PATH,
      sender_name=AGENT_NAME,
      to=[ORCHESTRATOR],
      thread_id=EPIC_ID,
      subject=f"[COMPLETE] {bead_id}",
      body_md=f"Bead {bead_id} closed."
    )
    
    # 6. Heartbeat (every 5 min)
    if time_since_last_heartbeat() > 5 * 60:
        send_heartbeat()
```

## Worker Session End

```python
# 1. Send track completion summary
send_message(
  project_key=PROJECT_PATH,
  sender_name=AGENT_NAME,
  to=[ORCHESTRATOR],
  thread_id=EPIC_ID,
  subject=f"[TRACK COMPLETE] Track {TRACK_N}",
  body_md=f"""
## Track {TRACK_N} Complete

- **Beads closed**: {len(assigned_beads)}
- **Files changed**: {list(changed_files)}
- **Duration**: {duration}
  """
)

# 2. Release file reservations
release_file_reservations(
  project_key=PROJECT_PATH,
  agent_name=AGENT_NAME
)
```

## Heartbeat Protocol

Workers send heartbeat every 5 minutes:

```python
send_message(
  project_key=PROJECT_PATH,
  sender_name=AGENT_NAME,
  to=[ORCHESTRATOR],
  thread_id=EPIC_ID,
  subject=f"[HEARTBEAT] Track {TRACK_N}",
  body_md=f"Still working. Current bead: {current_bead}"
)
```

Orchestrator marks worker stale if no heartbeat for 10 minutes.

## Cross-Track Dependency Notification

When completing a bead that other tracks depend on:

```python
# Check if this bead unblocks other tracks
if bead_id in blocking_beads:
    waiting_workers = find_waiting_workers(bead_id)
    for worker in waiting_workers:
        send_message(
          to=[worker],
          thread_id=EPIC_ID,
          subject=f"[DEP] {bead_id} COMPLETE - unblocked",
          body_md=f"You can now proceed with dependent beads."
        )
```

## AGENTS.md Guidance

Add to project AGENTS.md for worker sessions:

```markdown
## Worker Session Protocol

### On Start
1. Register: `register_agent(project_key, AGENT_NAME, program, model)`
2. Check inbox: `fetch_inbox` for dependency notifications
3. Reserve files: `file_reservation_paths(paths=[FILE_SCOPE])`

### During Work
1. Claim bead: `bd update <id> --status in_progress`
2. Do work
3. Close bead: `bd close <id> --reason completed`
4. Report: `send_message` with completion details
5. Heartbeat every 5 minutes

### On Complete
1. Send track summary
2. Release reservations: `release_file_reservations`
```
