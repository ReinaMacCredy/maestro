# Preflight Protocol Reference

Multi-session awareness protocol for orchestrator and conductor commands.

## Overview

Preflight (Phase 0) runs before `/conductor-implement` or `/conductor-orchestrate` to detect concurrent sessions and prevent conflicts.

## Phase 0: 4-Step Protocol

```
┌────────────────────────────────────────────────────────────────┐
│  PHASE 0: PREFLIGHT                                            │
├────────────────────────────────────────────────────────────────┤
│  Step 1: IDENTITY   - Register agent, generate session ID      │
│  Step 2: DETECT     - Scan for active sessions via Agent Mail  │
│  Step 3: DISPLAY    - Show session status, prompt if conflict  │
│  Step 4: PROCEED    - Continue or abort based on user choice   │
└────────────────────────────────────────────────────────────────┘
```

### Step 1: Identity

Register agent identity and generate session ID:

```python
# Generate unique session identity
import time
timestamp = int(time.time())
session_id = f"{BASE_AGENT}-{timestamp}"
display_name = f"{BASE_AGENT} (session {time.strftime('%H:%M')})"

# Register with Agent Mail
register_agent(
    project_key=PROJECT_PATH,
    name=session_id,        # Internal: unique
    program="amp",
    model=MODEL,
    task_description=f"Session started at {time.strftime('%H:%M')}"
)
```

### Step 2: Detect

Scan for active sessions:

```python
# Fetch inbox to discover other sessions
messages = fetch_inbox(
    project_key=PROJECT_PATH,
    agent_name=session_id,
    since_ts=datetime.now() - timedelta(hours=2),
    include_bodies=True
)

# Filter for session announcements
active_sessions = []
for msg in messages:
    if "[SESSION START]" in msg.subject:
        session_info = parse_session_start(msg)
        if not is_stale(session_info):
            active_sessions.append(session_info)
```

### Step 3: Display

Show session status using display formats (see below).

### Step 4: Proceed

Based on conflict detection:
- **No conflicts**: Proceed to Phase 1
- **Track conflict**: Prompt with [P]roceed/[S]witch/[W]ait
- **Stale session**: Prompt with [T]ake/[W]ait/[I]gnore

## Trigger Conditions

| Command | Preflight | Notes |
|---------|-----------|-------|
| `/conductor-implement` | ✅ Yes | Always runs Phase 0 |
| `/conductor-orchestrate` | ✅ Yes | Always runs Phase 0 |
| `ds` | ❌ Skip | Design sessions don't conflict |
| `bd ready` | ❌ Skip | Read-only command |
| `bd show` | ❌ Skip | Read-only command |
| `bd list` | ❌ Skip | Read-only command |

## Skip Rules

Preflight is skipped when:
1. Command is read-only (`bd ready`, `bd show`, `bd list`)
2. Command is design phase (`ds`, `/conductor-design`)
3. Agent Mail is unavailable (degrade to single-session mode)
4. Explicit `--skip-preflight` flag passed

## Agent Mail Integration

### Registration

```python
# Full registration with task context
register_agent(
    project_key=PROJECT_PATH,
    name=session_id,
    program="amp",
    model="claude-sonnet-4-20250514",
    task_description=f"Track: {track_id}, Started: {timestamp}"
)
```

### Inbox Fetch

```python
# Fetch with 2-hour lookback for session detection
messages = fetch_inbox(
    project_key=PROJECT_PATH,
    agent_name=session_id,
    since_ts="2025-01-01T08:00:00Z",  # 2 hours ago
    include_bodies=True,
    limit=50
)
```

## Error Handling

### Timeout Behavior

Agent Mail operations timeout after 3 seconds:

```python
try:
    result = register_agent(...)  # 3s timeout
except TimeoutError:
    # Degrade to single-session mode
    print("⚠️ Agent Mail timeout - proceeding without session detection")
    mode = "DEGRADED"
```

### Unavailable Service

```python
if not agent_mail_available():
    print("⚠️ Agent Mail unavailable - single-session mode")
    # Continue without multi-session awareness
    return {"mode": "DEGRADED", "sessions": []}
```

## Display Formats

### Active Sessions Box

```
┌─ ACTIVE SESSIONS ──────────────────────────────────────────┐
│                                                            │
│ 🟢 BlueLake (session 10:30) - active                       │
│    Track: cc-v2-integration                                │
│    Beads: bd-101 (in_progress)                             │
│    Files: src/api/**                                       │
│    Last seen: 2 min ago                                    │
│                                                            │
│ 🟡 GreenCastle (session 09:15) - stale (12 min)            │
│    Track: doc-updates                                      │
│    Beads: bd-201 (in_progress)                             │
│    Files: docs/**                                          │
│    Last seen: 12 min ago                                   │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Conflict Prompt

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

### Stale Takeover Prompt

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

## Session Status Indicators

| Icon | Status | Meaning |
|------|--------|---------|
| 🟢 | Active | Heartbeat within 5 min |
| 🟡 | Stale | No heartbeat for 5-10 min |
| 🔴 | Inactive | No heartbeat for >10 min |
| ⚪ | Unknown | No session data available |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `preflight_timeout` | 3s | Max time for Agent Mail operations |
| `session_lookback` | 2h | How far back to scan for sessions |
| `stale_threshold` | 10 min | When to consider session stale |
| `heartbeat_interval` | 5 min | Expected heartbeat frequency |

## References

- [session-identity.md](session-identity.md) - Identity format and collision handling
- [patterns/session-lifecycle.md](patterns/session-lifecycle.md) - Full lifecycle with multi-session
- [agent-coordination.md](agent-coordination.md) - Agent Mail coordination patterns
