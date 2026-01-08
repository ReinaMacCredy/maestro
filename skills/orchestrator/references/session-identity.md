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

```bash
# Generate unique session ID with collision retry
generate_session_id() {
    local base_agent="$1"
    local max_retries=3
    
    for attempt in $(seq 0 $((max_retries - 1))); do
        local timestamp=$(($(date +%s) + attempt))
        local session_id="${base_agent}-${timestamp}"
        
        # Try to register
        if bun toolboxes/agent-mail/agent-mail.js register-agent \
            --project-key "$PROJECT_PATH" \
            --name "$session_id" \
            --program "amp" \
            --model "$MODEL" \
            --task-description "Session registration" 2>/dev/null; then
            echo "$session_id"
            return 0
        fi
    done
    
    echo "Failed to generate unique session ID after $max_retries attempts" >&2
    return 1
}
```

### Collision Scenarios

| Scenario | Likelihood | Resolution |
|----------|------------|------------|
| Same second registration | Very low | Retry with +1 second |
| Recovered session | Low | Use existing or generate new |
| Clock sync issues | Very low | Use server timestamp |

## Agent Mail Profile Persistence

Session identity is persisted via `bun toolboxes/agent-mail/agent-mail.js register-agent`:

```bash
bun toolboxes/agent-mail/agent-mail.js register-agent \
    --project-key "$PROJECT_PATH" \
    --name "BlueLake-1735689600" \
    --program "amp" \
    --model "claude-opus-4-5@20251101" \
    --task-description "Track: cc-v2-integration, Epic: bd-100"
```

### Profile Fields

| Field | Value | Purpose |
|-------|-------|---------|
| `name` | Internal session ID | Unique identifier |
| `program` | `"amp"` | Client identifier |
| `model` | Model name | For coordination |
| `task_description` | Track/epic context | Human context |

### Profile Lifecycle

1. **Created**: At session start via `bun toolboxes/agent-mail/agent-mail.js register-agent --project-key ... --name ...`
2. **Updated**: On heartbeat via `bun toolboxes/agent-mail/agent-mail.js register-agent` (updates `last_active_ts`)
3. **Queried**: Via `bun toolboxes/agent-mail/agent-mail.js whois --project-key ... --agent-name ...` for session detection
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

### Via whois

```bash
# Get session profile
bun toolboxes/agent-mail/agent-mail.js whois \
    --project-key "$PROJECT_PATH" \
    --agent-name "BlueLake-1735689600" \
    --include-recent-commits true
```

### Via fetch_inbox

```bash
# Scan for session start messages
bun toolboxes/agent-mail/agent-mail.js fetch-inbox \
    --project-key "$PROJECT_PATH" \
    --agent-name "$MY_SESSION_ID" \
    --since-ts "$TWO_HOURS_AGO" \
    --include-bodies true
```

## Session Messages

### SESSION START

Sent when a session begins:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
    project_key:"$PROJECT_PATH" \
    sender_name:"$SESSION_ID" \
    to:'["Broadcast"]' \
    subject:"[SESSION START] BlueLake (session 10:30)" \
    body_md:"## Session Started

- **ID**: BlueLake-1735689600
- **Track**: cc-v2-integration
- **Beads**: bd-101, bd-102
- **Files**: skills/orchestrator/**"
```

### HEARTBEAT

Sent every 5 minutes:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
    project_key:"$PROJECT_PATH" \
    sender_name:"$SESSION_ID" \
    to:'["Broadcast"]' \
    subject:"[HEARTBEAT] BlueLake (session 10:30)" \
    body_md:"Working on bd-101. Files: skills/orchestrator/**"
```

### SESSION END

Sent on normal completion:

```bash
bun toolboxes/agent-mail/agent-mail.js send-message \
    project_key:"$PROJECT_PATH" \
    sender_name:"$SESSION_ID" \
    to:'["Broadcast"]' \
    subject:"[SESSION END] BlueLake (session 10:30)" \
    body_md:"## Session Complete

- **Duration**: 45 min
- **Beads closed**: bd-101, bd-102
- **Files released**: skills/orchestrator/**"
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
