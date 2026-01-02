# Worker Protocol v2

Comprehensive protocol for autonomous worker agents with self-propulsion, heartbeats, and Agent Mail coordination.

## Protocol Overview

Worker Protocol v2 introduces:
- **Self-propulsion** - Workers drive themselves through the full lifecycle
- **Heartbeat protocol** - 5-minute interval status signals
- **PING/PONG handling** - Liveness verification from orchestrator
- **BLOCKED behavior** - Graceful handling when work cannot proceed

## 7-Step Startup Sequence

Workers MUST execute these steps in order before beginning work:

```
┌─────────────────────────────────────────────────────────────┐
│  1. INITIALIZE     - macro_start_session() call            │
│  2. INBOX CHECK    - fetch_inbox() for pending messages    │
│  3. THREAD LOCATE  - Find epic thread from inbox           │
│  4. BEADS QUERY    - bd ready --json for available work    │
│  5. CLAIM          - bd update <id> --status in_progress   │
│  6. RESERVE        - Confirm file reservations active      │
│  7. EXECUTE        - Begin task work                       │
└─────────────────────────────────────────────────────────────┘
```

### Step 1: Initialize

```python
result = macro_start_session(
    human_key="{PROJECT_PATH}",
    program="amp",
    model="{MODEL}",
    file_reservation_paths=["{FILE_SCOPE}"],
    task_description="Worker for Track {TRACK_N}: {DESCRIPTION}"
)

if not result.agent:
    return {"status": "HALTED", "reason": "Session init failed"}
```

### Step 2: Inbox Check

Check for pending messages, especially:
- PING requests from orchestrator
- BLOCKED notifications from dependencies
- Updated instructions

```python
inbox = result.inbox  # Already populated by macro_start_session
for msg in inbox:
    if "[PING]" in msg.get("subject", ""):
        # Respond immediately with PONG
        reply_message(
            project_key="{PROJECT_PATH}",
            message_id=msg["id"],
            sender_name="{AGENT_NAME}",
            body_md="## PONG\nActive and ready."
        )
```

### Step 3: Thread Locate

Find the epic thread for coordination:

```python
epic_thread = None
for msg in inbox:
    if "{EPIC_ID}" in msg.get("thread_id", ""):
        epic_thread = msg.get("thread_id")
        break
# If not found, use EPIC_ID directly as thread_id
epic_thread = epic_thread or "{EPIC_ID}"
```

### Step 4: Beads Query

Query available work:

```bash
bd ready --json
```

Filter for assigned beads. If none available, check for blocking dependencies.

### Step 5: Claim

Claim each bead before working:

```bash
bd update {bead_id} --status in_progress
```

### Step 6: Reserve

Verify file reservations are active:

```python
# Reservations granted in macro_start_session result
if result.file_reservations.conflicts:
    # Report conflict and wait or HALT
    send_message(
        subject="[BLOCKED] File conflict",
        body_md=f"Conflicts: {result.file_reservations.conflicts}"
    )
```

### Step 7: Execute

Begin task work following TDD cycle (unless `--no-tdd`).

## Heartbeat Protocol

Workers MUST send heartbeats every **5 minutes** during execution.

### Heartbeat Message Format

```python
send_message(
    project_key="{PROJECT_PATH}",
    sender_name="{AGENT_NAME}",
    to=["{ORCHESTRATOR}"],
    thread_id="{EPIC_ID}",
    subject="[HEARTBEAT] Track {TRACK_N}",
    body_md="""
## Status
ACTIVE

## Current Work
- Bead: {current_bead}
- Phase: {RED|GREEN|REFACTOR}

## Progress
- Files modified: {count}
- Tests: {pass}/{total}
""",
    importance="low"
)
```

### Heartbeat Timing

| Duration | Required Heartbeats |
|----------|---------------------|
| < 5 min  | 0 (skip)            |
| 5-10 min | 1                   |
| 10-15 min | 2                  |
| > 15 min | Every 5 min         |

### Missed Heartbeat Consequences

If orchestrator receives no heartbeat for **10 minutes** (2x interval):

1. Orchestrator sends `[PING]` message
2. Worker has **5 minutes** to respond with `[PONG]`
3. No response → orchestrator marks worker as **STALE**

## PING/PONG Handling

### PING Request

Orchestrator sends when heartbeat is overdue:

```markdown
Subject: [PING] Track {TRACK_N} liveness check

## Request
Please confirm you are active.

## Deadline
Respond within 5 minutes or worker will be marked stale.
```

### PONG Response

Workers MUST check inbox during work and respond immediately:

```python
# Check inbox periodically (every 2-3 minutes recommended)
inbox = fetch_inbox(
    project_key="{PROJECT_PATH}",
    agent_name="{AGENT_NAME}",
    since_ts=last_check_ts
)

for msg in inbox:
    if "[PING]" in msg.get("subject", ""):
        reply_message(
            project_key="{PROJECT_PATH}",
            message_id=msg["id"],
            sender_name="{AGENT_NAME}",
            body_md="""
## PONG
Active and working.

## Current State
- Bead: {current_bead}
- Phase: {phase}
- Last action: {last_action}
"""
        )
        acknowledge_message(
            project_key="{PROJECT_PATH}",
            agent_name="{AGENT_NAME}",
            message_id=msg["id"]
        )
```

### Inline PING/PONG Check

Add this check within your main work loop:

```python
def check_for_ping():
    inbox = fetch_inbox(
        project_key="{PROJECT_PATH}",
        agent_name="{AGENT_NAME}",
        since_ts=last_ping_check,
        limit=5
    )
    for msg in inbox:
        if "[PING]" in msg.get("subject", ""):
            respond_pong(msg)
    return datetime.now().isoformat()

# In main loop
last_ping_check = check_for_ping()  # Every 2-3 minutes
```

## BLOCKED Behavior

When a worker cannot proceed, it MUST follow this protocol:

### 1. Report Block Immediately

```python
send_message(
    project_key="{PROJECT_PATH}",
    sender_name="{AGENT_NAME}",
    to=["{ORCHESTRATOR}"],
    thread_id="{EPIC_ID}",
    subject="[BLOCKED] Track {TRACK_N}: {REASON}",
    body_md="""
## Blocker Type
{dependency|conflict|error|external}

## Details
{Detailed description of what's blocking}

## Blocked Beads
- {bead_id}: {reason}

## Attempted Resolution
- {What was tried}
- {Why it failed}

## Recommended Action
{What orchestrator should do}
""",
    importance="high",
    ack_required=True
)
```

### 2. Mark Bead as Blocked

```bash
bd close {bead_id} --reason blocked
```

### 3. Continue or Wait

| Scenario | Action |
|----------|--------|
| Other beads available | Continue with unblocked beads |
| All beads blocked | Send final report and exit |
| Waiting for dependency | Poll inbox every 30 seconds |

### 4. Dependency Wait Pattern

```python
def wait_for_dependency(dependency_id, timeout_minutes=30):
    """Poll inbox for dependency completion."""
    start = time.time()
    while (time.time() - start) < timeout_minutes * 60:
        inbox = fetch_inbox(
            project_key="{PROJECT_PATH}",
            agent_name="{AGENT_NAME}",
            since_ts=last_check
        )
        for msg in inbox:
            if "[DEP]" in msg.get("subject", "") and dependency_id in msg.get("body", ""):
                return True
            if "[PING]" in msg.get("subject", ""):
                respond_pong(msg)
        time.sleep(30)  # Poll every 30 seconds
    return False  # Timeout
```

## Complete Worker Lifecycle

```
START
  │
  ├─→ [1-7] Startup Sequence
  │     │
  │     └─→ Fail? → HALT with reason
  │
  ├─→ WORK LOOP ←────────────────────┐
  │     │                             │
  │     ├─→ Check inbox (PING?)       │
  │     ├─→ Send heartbeat (5min)     │
  │     ├─→ Execute bead              │
  │     │     │                       │
  │     │     ├─→ Success → Close bead│
  │     │     └─→ Blocked → Report    │
  │     │                             │
  │     └─→ More beads? ─────────────┘
  │
  ├─→ REPORT
  │     └─→ send_message([TRACK COMPLETE])
  │
  └─→ CLEANUP
        └─→ release_file_reservations()
```

## Message Catalog (Worker)

| Subject Pattern | When | Importance |
|-----------------|------|------------|
| `[HEARTBEAT] Track N` | Every 5 min | low |
| `[PONG] ...` | Reply to PING | normal |
| `[BLOCKED] Track N: ...` | Cannot proceed | high |
| `[DEP] Task X complete` | Dependency satisfied | normal |
| `[CONTEXT] Bead X complete` | Bead learnings | low |
| `[TRACK COMPLETE] Track N` | All beads done | normal |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `heartbeat_interval` | 300s | Seconds between heartbeats |
| `ping_timeout` | 300s | Seconds to respond to PING |
| `inbox_poll_interval` | 180s | Seconds between inbox checks |
| `dependency_poll_interval` | 30s | Seconds between dependency checks |
| `dependency_timeout` | 1800s | Max wait for dependency |

## Related

- [worker-prompt-v2.md](worker-prompt-v2.md) - Worker prompt template using this protocol
- [worker-prompt.md](worker-prompt.md) - Original v1 prompt template
- [agent-coordination.md](agent-coordination.md) - Cross-track coordination
- [summary-protocol.md](summary-protocol.md) - Completion message format
