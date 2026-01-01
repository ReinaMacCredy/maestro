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

## Multi-Session Awareness

When multiple sessions may be active, workers use session announcement messages.

### SESSION START Message

Sent immediately after registration:

```python
send_message(
    project_key=PROJECT_PATH,
    sender_name=AGENT_NAME,
    to=["Broadcast"],  # Or orchestrator if known
    thread_id=EPIC_ID,
    subject=f"[SESSION START] {DISPLAY_NAME}",
    body_md=f"""
## Session Started

- **ID**: {SESSION_ID}
- **Track**: {TRACK_ID}
- **Beads**: {', '.join(assigned_beads)}
- **Files**: {FILE_SCOPE}
- **Started**: {timestamp}
"""
)
```

### HEARTBEAT Message

Sent every 5 minutes during active work:

```python
send_message(
    project_key=PROJECT_PATH,
    sender_name=AGENT_NAME,
    to=["Broadcast"],
    thread_id=EPIC_ID,
    subject=f"[HEARTBEAT] {DISPLAY_NAME}",
    body_md=f"Working on {current_bead}. Files: {FILE_SCOPE}"
)
```

### SESSION END Message

Sent on normal session completion:

```python
send_message(
    project_key=PROJECT_PATH,
    sender_name=AGENT_NAME,
    to=["Broadcast"],
    thread_id=EPIC_ID,
    subject=f"[SESSION END] {DISPLAY_NAME}",
    body_md=f"""
## Session Complete

- **Duration**: {duration}
- **Beads closed**: {', '.join(closed_beads)}
- **Files released**: {FILE_SCOPE}
"""
)
```

### Stale Session Detection

A session is considered stale when no heartbeat received for >10 minutes:

```python
def is_stale(session: Session) -> bool:
    """Check if session is stale (no activity for >10 min)."""
    stale_threshold = timedelta(minutes=10)
    return datetime.now() - session.last_heartbeat > stale_threshold

def detect_stale_sessions(messages: list) -> list[Session]:
    """Scan inbox for stale sessions."""
    sessions = {}
    
    for msg in messages:
        if "[SESSION START]" in msg.subject:
            session = parse_session(msg)
            sessions[session.id] = session
        elif "[HEARTBEAT]" in msg.subject:
            session_id = extract_session_id(msg)
            if session_id in sessions:
                sessions[session_id].last_heartbeat = msg.created_ts
        elif "[SESSION END]" in msg.subject:
            session_id = extract_session_id(msg)
            sessions.pop(session_id, None)
    
    return [s for s in sessions.values() if is_stale(s)]
```

### Takeover Flow

When a stale session is detected, the new session can take over:

```
┌─ STALE SESSION DETECTED ───────────────────────────────────┐
│ GreenCastle (session 09:15) inactive for 12 minutes        │
│                                                            │
│ Reserved files: skills/orchestrator/**                     │
│ Claimed beads: bd-201 (in_progress)                        │
│                                                            │
│ ⚠️  Warning: May have uncommitted work                      │
│                                                            │
│ [T]ake over - release reservations, reset beads to open    │
│ [W]ait - check again in 5 min                              │
│ [I]gnore - proceed without their files/beads               │
└────────────────────────────────────────────────────────────┘
```

#### Takeover Actions

| Option | Action | When to Use |
|--------|--------|-------------|
| **[T]ake** | Force-release reservations, reset beads to `open` | Confident session is abandoned |
| **[W]ait** | Re-check in 5 minutes | Session might recover |
| **[I]gnore** | Proceed without their resources | Work on different files/beads |

#### Takeover Implementation

```python
def takeover_session(stale_session: Session) -> None:
    """Take over a stale session's resources."""
    
    # 1. Force-release file reservations
    for reservation_id in stale_session.reservation_ids:
        force_release_file_reservation(
            project_key=PROJECT_PATH,
            agent_name=MY_AGENT_NAME,
            file_reservation_id=reservation_id,
            note=f"Takeover by {MY_AGENT_NAME} - session stale for {stale_duration}",
            notify_previous=True
        )
    
    # 2. Reset beads to open status
    for bead_id in stale_session.beads_claimed:
        bash(f"bd update {bead_id} --status open --notes 'Reset by {MY_AGENT_NAME} takeover'")
    
    # 3. Announce takeover
    send_message(
        project_key=PROJECT_PATH,
        sender_name=MY_AGENT_NAME,
        to=[stale_session.id],  # Notify original owner if they return
        subject=f"[TAKEOVER] Resources from {stale_session.display}",
        body_md=f"""
## Session Takeover

Took over resources from stale session {stale_session.display}.

- **Reservations released**: {len(stale_session.reservation_ids)}
- **Beads reset**: {', '.join(stale_session.beads_claimed)}
- **Reason**: Inactive for {stale_duration}
"""
    )
```

### Conflict Resolution

When multiple active sessions target the same resources:

```
┌─ CONFLICTS DETECTED ───────────────────────────────────────┐
│ ⚠️  TRACK CONFLICT                                         │
│     BlueLake (session 10:30) is already on cc-v2-integration│
│                                                            │
│ Options:                                                   │
│ [P]roceed anyway - work on different files/beads           │
│ [S]witch track - pick a different track                    │
│ [W]ait - let other session finish first                    │
└────────────────────────────────────────────────────────────┘
```

#### Resolution Strategies

| Conflict Type | Resolution | Notes |
|---------------|------------|-------|
| Same track | Coordinate via file scope | Different files = OK |
| Same files | Wait or takeover | Depends on staleness |
| Same beads | Wait | Never claim same bead |

## AGENTS.md Guidance

Add to project AGENTS.md for worker sessions:

```markdown
## Worker Session Protocol

### On Start
1. Register: `register_agent(project_key, AGENT_NAME, program, model)`
2. Announce: `send_message` with `[SESSION START]`
3. Check inbox: `fetch_inbox` for dependency notifications
4. Reserve files: `file_reservation_paths(paths=[FILE_SCOPE])`

### During Work
1. Claim bead: `bd update <id> --status in_progress`
2. Do work
3. Close bead: `bd close <id> --reason completed`
4. Report: `send_message` with completion details
5. Heartbeat: `send_message` with `[HEARTBEAT]` every 5 minutes

### On Complete
1. Send track summary
2. Announce: `send_message` with `[SESSION END]`
3. Release reservations: `release_file_reservations`
```

## References

- [../preflight.md](../preflight.md) - Preflight protocol for session detection
- [../session-identity.md](../session-identity.md) - Session identity format
