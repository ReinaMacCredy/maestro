# Session Lifecycle Pattern

Worker session lifecycle with Agent Mail coordination.

## Overview

Each worker maintains its own session lifecycle:
1. Register with Agent Mail
2. Work on assigned beads
3. Report progress via messages
4. Complete and send summary

## Worker Session Start

```bash
# 1. Register agent identity
bun toolboxes/agent-mail/agent-mail.js register-agent \
  project_key:"$PROJECT_PATH" \
  name:"$AGENT_NAME" \
  program:"amp" \
  model:"$MODEL" \
  task_description:"Worker for Track $TRACK_N"

# 2. Check for cross-track dependency notifications
bun toolboxes/agent-mail/agent-mail.js fetch-inbox \
  project_key:"$PROJECT_PATH" \
  agent_name:"$AGENT_NAME" \
  include_bodies:true

# 3. Filter for dependency completions (parse JSON output)
# Look for [DEP] ... COMPLETE patterns in subject
```

## Worker Session Loop

```bash
for bead_id in $ASSIGNED_BEADS; do
    # 1. Reserve files for this bead
    bun toolboxes/agent-mail/agent-mail.js file-reservation-paths \
      project_key:"$PROJECT_PATH" \
      agent_name:"$AGENT_NAME" \
      paths:"[\"$FILE_SCOPE\"]" \
      ttl_seconds:3600
    
    # 2. Claim bead
    bd update "$bead_id" --status in_progress
    
    # 3. Do work
    # implement_task "$bead_id"
    
    # 4. Close bead
    bd close "$bead_id" --reason completed
    
    # 5. Report completion
    bun toolboxes/agent-mail/agent-mail.js send-message \
      project_key:"$PROJECT_PATH" \
      sender_name:"$AGENT_NAME" \
      to:"[\"$ORCHESTRATOR\"]" \
      thread_id:"$EPIC_ID" \
      subject:"[COMPLETE] $bead_id" \
      body_md:"Bead $bead_id closed."
    
    # 6. Heartbeat (every 5 min) - handled separately
done
```

## Worker Session End

```bash
# 1. Send track completion summary
bun toolboxes/agent-mail/agent-mail.js send-message \
  project_key:"$PROJECT_PATH" \
  sender_name:"$AGENT_NAME" \
  to:"[\"$ORCHESTRATOR\"]" \
  thread_id:"$EPIC_ID" \
  subject:"[TRACK COMPLETE] Track $TRACK_N" \
  body_md:"## Track $TRACK_N Complete

- **Beads closed**: $BEAD_COUNT
- **Files changed**: $CHANGED_FILES
- **Duration**: $DURATION"

# 2. Release file reservations
bun toolboxes/agent-mail/agent-mail.js release-file-reservations \
  project_key:"$PROJECT_PATH" \
  agent_name:"$AGENT_NAME"
```

## Heartbeat Protocol

Workers send heartbeat every 5 minutes:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
  project_key:"$PROJECT_PATH" \
  sender_name:"$AGENT_NAME" \
  to:'["$ORCHESTRATOR"]' \
  thread_id:"$EPIC_ID" \
  subject:"[HEARTBEAT] Track $TRACK_N" \
  body_md:"Still working. Current bead: $CURRENT_BEAD"
```

Orchestrator marks worker stale if no heartbeat for 10 minutes.

## Cross-Track Dependency Notification

When completing a bead that other tracks depend on:

```bash
# Check if this bead unblocks other tracks
# If bead_id is in blocking_beads, notify waiting workers
for worker in $WAITING_WORKERS; do
    bun toolboxes/agent-mail/agent-mail.js send-message \
      project_key:"$PROJECT_PATH" \
      sender_name:"$AGENT_NAME" \
      to:"[\"$worker\"]" \
      thread_id:"$EPIC_ID" \
      subject:"[DEP] $BEAD_ID COMPLETE - unblocked" \
      body_md:"You can now proceed with dependent beads."
done
```

## Multi-Session Awareness

When multiple sessions may be active, workers use session announcement messages.

### SESSION START Message

Sent immediately after registration:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
    project_key:"$PROJECT_PATH" \
    sender_name:"$AGENT_NAME" \
    to:'["Broadcast"]' \
    thread_id:"$EPIC_ID" \
    subject:"[SESSION START] $DISPLAY_NAME" \
    body_md:"## Session Started

- **ID**: $SESSION_ID
- **Track**: $TRACK_ID
- **Beads**: $ASSIGNED_BEADS
- **Files**: $FILE_SCOPE
- **Started**: $TIMESTAMP"
```

### HEARTBEAT Message

Sent every 5 minutes during active work:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
    project_key:"$PROJECT_PATH" \
    sender_name:"$AGENT_NAME" \
    to:'["Broadcast"]' \
    thread_id:"$EPIC_ID" \
    subject:"[HEARTBEAT] $DISPLAY_NAME" \
    body_md:"Working on $CURRENT_BEAD. Files: $FILE_SCOPE"
```

### SESSION END Message

Sent on normal session completion:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
    project_key:"$PROJECT_PATH" \
    sender_name:"$AGENT_NAME" \
    to:'["Broadcast"]' \
    thread_id:"$EPIC_ID" \
    subject:"[SESSION END] $DISPLAY_NAME" \
    body_md:"## Session Complete

- **Duration**: $DURATION
- **Beads closed**: $CLOSED_BEADS
- **Files released**: $FILE_SCOPE"
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

```bash
# Take over a stale session's resources
takeover_session() {
    local stale_session_id="$1"
    
    # 1. Force-release file reservations
    for reservation_id in $RESERVATION_IDS; do
        bun toolboxes/agent-mail/agent-mail.js force-release-file-reservation \
            project_key:"$PROJECT_PATH" \
            agent_name:"$MY_AGENT_NAME" \
            file_reservation_id:"$reservation_id" \
            note:"Takeover by $MY_AGENT_NAME - session stale for $STALE_DURATION" \
            notify_previous:true
    done
    
    # 2. Reset beads to open status
    for bead_id in $BEADS_CLAIMED; do
        bd update "$bead_id" --status open --notes "Reset by $MY_AGENT_NAME takeover"
    done
    
    # 3. Announce takeover
    bun toolboxes/agent-mail/agent-mail.js send-message \
        project_key:"$PROJECT_PATH" \
        sender_name:"$MY_AGENT_NAME" \
        to:"[\"$stale_session_id\"]" \
        subject:"[TAKEOVER] Resources from $STALE_SESSION_DISPLAY" \
        body_md:"## Session Takeover

Took over resources from stale session $STALE_SESSION_DISPLAY.

- **Reservations released**: $RESERVATION_COUNT
- **Beads reset**: $BEADS_CLAIMED
- **Reason**: Inactive for $STALE_DURATION"
}
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
1. Register: `bun toolboxes/agent-mail/agent-mail.js register-agent project_key:... name:$AGENT_NAME program:... model:...`
2. Announce: `bun toolboxes/agent-mail/agent-mail.js send-message` with `[SESSION START]`
3. Check inbox: `bun toolboxes/agent-mail/agent-mail.js fetch-inbox` for dependency notifications
4. Reserve files: `bun toolboxes/agent-mail/agent-mail.js file-reservation-paths paths:'["$FILE_SCOPE"]'`

### During Work
1. Claim bead: `bd update <id> --status in_progress`
2. Do work
3. Close bead: `bd close <id> --reason completed`
4. Report: `bun toolboxes/agent-mail/agent-mail.js send-message` with completion details
5. Heartbeat: `bun toolboxes/agent-mail/agent-mail.js send-message` with `[HEARTBEAT]` every 5 minutes

### On Complete
1. Send track summary
2. Announce: `bun toolboxes/agent-mail/agent-mail.js send-message` with `[SESSION END]`
3. Release reservations: `bun toolboxes/agent-mail/agent-mail.js release-file-reservations`
```

## References

- [../preflight.md](../preflight.md) - Preflight protocol for session detection
- [../session-identity.md](../session-identity.md) - Session identity format
