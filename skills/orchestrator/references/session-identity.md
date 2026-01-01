# Session Identity Reference

Agent identity format and management for multi-session coordination.

## Identity Format

### Internal vs Display

| Type | Format | Purpose |
|------|--------|---------|
| **Internal** | `{BaseAgent}-{timestamp}` | Unique identifier for Agent Mail |
| **Display** | `{BaseAgent} (session HH:MM)` | Human-readable for UI |

### Examples

```
Internal: BlueLake-1735689600
Display:  BlueLake (session 10:30)

Internal: GreenCastle-1735686000
Display:  GreenCastle (session 09:20)
```

### Generation

```python
import time

BASE_AGENT = "BlueLake"  # Adjective+Noun format
timestamp = int(time.time())

# Internal: unique for Agent Mail registration
internal_id = f"{BASE_AGENT}-{timestamp}"

# Display: human-readable
display_id = f"{BASE_AGENT} (session {time.strftime('%H:%M')})"
```

## Collision Handling

If a session ID already exists (extremely rare with timestamps):

```python
def generate_session_id(base_agent: str, max_retries: int = 3) -> str:
    """Generate unique session ID with collision retry."""
    for attempt in range(max_retries):
        timestamp = int(time.time()) + attempt  # Increment on retry
        session_id = f"{base_agent}-{timestamp}"
        
        try:
            register_agent(
                project_key=PROJECT_PATH,
                name=session_id,
                program="amp",
                model=MODEL,
                task_description="Session registration"
            )
            return session_id  # Success
        except AgentExistsError:
            continue  # Retry with incremented timestamp
    
    raise RuntimeError(f"Failed to generate unique session ID after {max_retries} attempts")
```

### Collision Scenarios

| Scenario | Likelihood | Resolution |
|----------|------------|------------|
| Same second registration | Very low | Retry with +1 second |
| Recovered session | Low | Use existing or generate new |
| Clock sync issues | Very low | Use server timestamp |

## Agent Mail Profile Persistence

Session identity is persisted via `register_agent()`:

```python
register_agent(
    project_key=PROJECT_PATH,
    name="BlueLake-1735689600",
    program="amp",
    model="claude-sonnet-4-20250514",
    task_description="Track: cc-v2-integration, Epic: bd-100"
)
```

### Profile Fields

| Field | Value | Purpose |
|-------|-------|---------|
| `name` | Internal session ID | Unique identifier |
| `program` | `"amp"` | Client identifier |
| `model` | Model name | For coordination |
| `task_description` | Track/epic context | Human context |

### Profile Lifecycle

1. **Created**: At session start via `register_agent()`
2. **Updated**: On heartbeat via `register_agent()` (updates `last_active_ts`)
3. **Queried**: Via `whois()` for session detection
4. **Retained**: Profiles persist for audit trail

## Session Data Model

Complete session state tracked during coordination:

```python
@dataclass
class Session:
    # Identity
    id: str                    # Internal: "BlueLake-1735689600"
    display: str               # Display: "BlueLake (session 10:30)"
    base_agent: str            # Base: "BlueLake"
    
    # Timestamps
    created_ts: datetime       # Session start
    last_heartbeat: datetime   # Last activity
    
    # Work context
    track: str | None          # Track ID if assigned
    beads_claimed: list[str]   # Beads with in_progress status
    files_reserved: list[str]  # File reservation patterns
    
    # Status
    status: Literal[           # Current state
        "active",              # Heartbeat within 5 min
        "stale",               # No heartbeat for 5-10 min
        "inactive",            # No heartbeat for >10 min
        "completed"            # Session ended normally
    ]
```

### Status Transitions

```
┌─────────┐   register   ┌────────┐   5 min no   ┌───────┐   5 min no   ┌──────────┐
│  (new)  │ ──────────► │ active │ ──heartbeat─► │ stale │ ──heartbeat─► │ inactive │
└─────────┘              └────────┘               └───────┘               └──────────┘
                              │                       │
                              │ heartbeat             │ heartbeat
                              ▼                       ▼
                         ┌────────┐              ┌────────┐
                         │ active │              │ active │
                         └────────┘              └────────┘
                              │
                              │ send_message "[SESSION END]"
                              ▼
                        ┌───────────┐
                        │ completed │
                        └───────────┘
```

## Querying Sessions

### Via whois()

```python
# Get session profile
profile = whois(
    project_key=PROJECT_PATH,
    agent_name="BlueLake-1735689600",
    include_recent_commits=True
)

# Extract session info
session = Session(
    id=profile["name"],
    display=format_display(profile["name"]),
    base_agent=profile["name"].split("-")[0],
    created_ts=profile["inception_ts"],
    last_heartbeat=profile["last_active_ts"],
    track=extract_track(profile["task_description"]),
    # ... other fields from Agent Mail state
)
```

### Via fetch_inbox()

```python
# Scan for session start messages
messages = fetch_inbox(
    project_key=PROJECT_PATH,
    agent_name=MY_SESSION_ID,
    since_ts=two_hours_ago,
    include_bodies=True
)

active_sessions = []
for msg in messages:
    if "[SESSION START]" in msg.subject:
        session = parse_session_from_message(msg)
        active_sessions.append(session)
```

## Session Messages

### SESSION START

Sent when a session begins:

```python
send_message(
    project_key=PROJECT_PATH,
    sender_name=session_id,
    to=["Broadcast"],  # Or specific orchestrator
    subject="[SESSION START] BlueLake (session 10:30)",
    body_md="""
## Session Started

- **ID**: BlueLake-1735689600
- **Track**: cc-v2-integration
- **Beads**: bd-101, bd-102
- **Files**: skills/orchestrator/**
"""
)
```

### HEARTBEAT

Sent every 5 minutes:

```python
send_message(
    project_key=PROJECT_PATH,
    sender_name=session_id,
    to=["Broadcast"],
    subject="[HEARTBEAT] BlueLake (session 10:30)",
    body_md="Working on bd-101. Files: skills/orchestrator/**"
)
```

### SESSION END

Sent on normal completion:

```python
send_message(
    project_key=PROJECT_PATH,
    sender_name=session_id,
    to=["Broadcast"],
    subject="[SESSION END] BlueLake (session 10:30)",
    body_md="""
## Session Complete

- **Duration**: 45 min
- **Beads closed**: bd-101, bd-102
- **Files released**: skills/orchestrator/**
"""
)
```

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `session_id_format` | `{base}-{timestamp}` | Internal ID template |
| `display_format` | `{base} (session HH:MM)` | Display template |
| `collision_retries` | 3 | Max retries on collision |
| `profile_retention` | 7 days | How long to keep profiles |

## References

- [preflight.md](preflight.md) - Preflight protocol using session identity
- [patterns/session-lifecycle.md](patterns/session-lifecycle.md) - Full session lifecycle
- [agent-coordination.md](agent-coordination.md) - Agent Mail coordination
