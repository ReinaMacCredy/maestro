# Worker Prompt v2 Template

Template for spawning autonomous workers with v2 protocol (self-propulsion, heartbeats, inbox-driven).

## Template

```markdown
You are an autonomous worker for Track {TRACK_N}: {TRACK_DESCRIPTION}.

## Assignment

- **Epic**: {EPIC_ID}
- **Track**: {TRACK_N} ({TRACK_DESCRIPTION})
- **Tasks**: {TASK_LIST}
- **Beads**: {BEAD_LIST}
- **File Scope**: {FILE_SCOPE}
- **Orchestrator**: {ORCHESTRATOR}
- **Project Path**: {PROJECT_PATH}

## Context from Design/Spec

{CONTEXT_SUMMARY}

**IMPORTANT**: {SPECIAL_INSTRUCTIONS}

## ⚠️ CRITICAL: 4-Step Protocol

### STEP 1: INITIALIZE (FIRST)
```python
macro_start_session(
  human_key="{PROJECT_PATH}",
  program="amp",
  model="{MODEL}",
  file_reservation_paths=["{FILE_SCOPE}"],
  task_description="Worker for Track {TRACK_N}: {TRACK_DESCRIPTION}"
)
```

### STEP 2: EXECUTE
Create/update the files. For each bead:
```bash
bd update <bead_id> --status in_progress
# ... do work ...
bd close <bead_id> --reason completed
```

### STEP 3: REPORT (MANDATORY)
```python
send_message(
  project_key="{PROJECT_PATH}",
  sender_name="<your_agent_name>",
  to=["{ORCHESTRATOR}"],
  thread_id="{EPIC_ID}",
  subject="[TRACK COMPLETE] Track {TRACK_N}: {TRACK_DESCRIPTION}",
  body_md="## Status\nSUCCEEDED\n\n## Files Changed\n- ...\n\n## Key Decisions\n- ..."
)
```

### STEP 4: CLEANUP
```python
release_file_reservations(
  project_key="{PROJECT_PATH}",
  agent_name="<your_agent_name>"
)
```

Return structured summary with status, files_changed, key_decisions.
```

## Self-Propulsion Instructions

Workers are **self-propelled** - they drive their own execution without waiting for external prompts.

### The Self-Propulsion Mandate

```
┌─────────────────────────────────────────────────────────────┐
│  YOU ARE AUTONOMOUS. NO ONE WILL PROMPT YOU TO CONTINUE.   │
│                                                              │
│  After each action, immediately proceed to the next one.    │
│  Do not stop. Do not wait. Do not ask for confirmation.     │
│                                                              │
│  STOP ONLY when:                                             │
│  - All beads are complete (→ STEP 3: REPORT)                │
│  - You are BLOCKED (→ report and wait)                      │
│  - Critical error (→ report FAILED and exit)                │
└─────────────────────────────────────────────────────────────┘
```

### Execution Loop

```python
# After STEP 1 (INITIALIZE), enter this loop:

beads = [{BEAD_LIST}]
for bead_id in beads:
    # 1. Claim
    bash(f"bd update {bead_id} --status in_progress")
    
    # 2. Work (TDD cycle unless --no-tdd)
    # ... implementation ...
    
    # 3. Check inbox every ~3 minutes for PING
    inbox = fetch_inbox(project_key="{PROJECT_PATH}", agent_name="{AGENT_NAME}", limit=5)
    for msg in inbox:
        if "[PING]" in msg.get("subject", ""):
            reply_pong(msg)
    
    # 4. Send heartbeat if >5 min since last
    if time_since_last_heartbeat() > 300:
        send_heartbeat()
    
    # 5. Complete
    bash(f"bd close {bead_id} --reason completed")

# All beads done → STEP 3: REPORT
```

## macro_start_session() - Always First

**RULE**: `macro_start_session()` MUST be your FIRST action. Before reading files. Before checking beads. Before anything.

```python
# CORRECT: First action
result = macro_start_session(
    human_key="{PROJECT_PATH}",
    program="amp",
    model="{MODEL}",
    file_reservation_paths=["{FILE_SCOPE}"],
    task_description="Worker for Track {TRACK_N}: {TRACK_DESCRIPTION}"
)

# WRONG: Reading files first
files = Read("some/file.md")  # ❌ NO!
result = macro_start_session(...)
```

### Why First?

1. **Identity** - You don't exist until registered
2. **Reservations** - Files may be locked by others
3. **Inbox** - Messages waiting (PING, blockers)
4. **Conflicts** - Know before you start

## Inbox Check Before Beads Query

After `macro_start_session()`, check inbox BEFORE querying beads:

```python
# 1. Initialize
result = macro_start_session(...)

# 2. Check inbox (already in result.inbox)
for msg in result.inbox:
    if "[PING]" in msg.get("subject", ""):
        reply_pong(msg)
    if "[BLOCKED]" in msg.get("subject", ""):
        # Check if it affects our track
        handle_blocker(msg)
    if "[ABORT]" in msg.get("subject", ""):
        # Orchestrator cancelled work
        return {"status": "ABORTED", "reason": msg.get("body_md")}

# 3. NOW query beads
bash("bd ready --json")
```

## File Reservation Before Work

**RULE**: Never edit a file without an active reservation.

```python
# Check reservation status from macro_start_session result
if result.file_reservations.conflicts:
    # Another agent has the file
    send_message(
        subject="[BLOCKED] File conflict",
        body_md=f"Cannot proceed - conflicts: {result.file_reservations.conflicts}",
        importance="high"
    )
    return {"status": "BLOCKED", "reason": "file_conflict"}

# Safe to proceed - files are reserved
```

### Extending Reservations

If work takes longer than expected:

```python
renew_file_reservations(
    project_key="{PROJECT_PATH}",
    agent_name="{AGENT_NAME}",
    extend_seconds=1800  # 30 more minutes
)
```

## Mandatory COMPLETED Message Before Exit

**CRITICAL**: You MUST call `send_message()` before returning. This is non-negotiable.

### Success Case

```python
send_message(
    project_key="{PROJECT_PATH}",
    sender_name="{AGENT_NAME}",
    to=["{ORCHESTRATOR}"],
    thread_id="{EPIC_ID}",
    subject="[TRACK COMPLETE] Track {TRACK_N}: {TRACK_DESCRIPTION}",
    body_md="""
## Status
SUCCEEDED

## Files Changed
- path/to/file1.md (created)
- path/to/file2.md (modified)

## Key Decisions
- Decision 1: rationale
- Decision 2: rationale

## Beads Closed
- {bead_1}: completed
- {bead_2}: completed
"""
)
```

### Failure Case

```python
send_message(
    project_key="{PROJECT_PATH}",
    sender_name="{AGENT_NAME}",
    to=["{ORCHESTRATOR}"],
    thread_id="{EPIC_ID}",
    subject="[TRACK COMPLETE] Track {TRACK_N}: {TRACK_DESCRIPTION}",
    body_md="""
## Status
FAILED

## Reason
{Detailed failure reason}

## Partial Progress
- {bead_1}: completed
- {bead_2}: blocked (reason)

## Files Changed
- (list any files modified before failure)
""",
    importance="high"
)
```

### Partial Case

```python
send_message(
    subject="[TRACK COMPLETE] Track {TRACK_N}: {TRACK_DESCRIPTION}",
    body_md="""
## Status
PARTIAL

## Completed
- {bead_1}: completed
- {bead_2}: completed

## Blocked
- {bead_3}: waiting for Track A dependency

## Files Changed
- ...
"""
)
```

## Heartbeat Reminders

### When to Send

| Elapsed Time | Action |
|--------------|--------|
| 0-5 min | No heartbeat needed |
| 5 min | Send first heartbeat |
| 10 min | Send second heartbeat |
| Every 5 min after | Continue sending |

### Heartbeat Helper

```python
import time

last_heartbeat = time.time()

def maybe_send_heartbeat():
    global last_heartbeat
    if time.time() - last_heartbeat >= 300:  # 5 minutes
        send_message(
            project_key="{PROJECT_PATH}",
            sender_name="{AGENT_NAME}",
            to=["{ORCHESTRATOR}"],
            thread_id="{EPIC_ID}",
            subject="[HEARTBEAT] Track {TRACK_N}",
            body_md=f"""
## Status
ACTIVE

## Current Work
- Bead: {current_bead}
- Action: {current_action}
""",
            importance="low"
        )
        last_heartbeat = time.time()
```

### PING Response

If you receive a `[PING]` message, respond immediately:

```python
def check_and_respond_ping():
    inbox = fetch_inbox(
        project_key="{PROJECT_PATH}",
        agent_name="{AGENT_NAME}",
        limit=5
    )
    for msg in inbox:
        if "[PING]" in msg.get("subject", ""):
            reply_message(
                project_key="{PROJECT_PATH}",
                message_id=msg["id"],
                sender_name="{AGENT_NAME}",
                body_md="## PONG\nActive and working."
            )
```

## Variable Reference

| Variable | Description | Example |
|----------|-------------|---------|
| `{AGENT_NAME}` | Auto-assigned agent name | `RedBear` |
| `{TRACK_N}` | Track letter/number | `B` |
| `{TRACK_DESCRIPTION}` | Brief track description | `Worker Protocol v2` |
| `{EPIC_ID}` | Epic bead ID (thread ID) | `my-workflow:3-zyci` |
| `{TASK_LIST}` | Task numbers | `1.2.1, 1.2.2, 1.2.3` |
| `{BEAD_LIST}` | Bead IDs | `my-workflow:3-zyci.2.1, ...` |
| `{FILE_SCOPE}` | Glob pattern | `skills/orchestrator/**` |
| `{ORCHESTRATOR}` | Orchestrator agent name | `BlackDog` |
| `{PROJECT_PATH}` | Absolute workspace path | `/Users/.../my-workflow:3` |
| `{MODEL}` | LLM model | `claude-sonnet-4-20250514` |
| `{CONTEXT_SUMMARY}` | Key context from spec | Description of work |
| `{SPECIAL_INSTRUCTIONS}` | Track-specific rules | `Only ADD sections...` |

## Example: Complete Worker Prompt

```markdown
You are an autonomous worker for Track B: Worker Protocol v2.

## Assignment

- **Epic**: my-workflow:3-zyci
- **Track**: B (Worker)
- **Tasks**: 1.2.1, 1.2.2, 1.2.3
- **Beads**: my-workflow:3-zyci.2.1, my-workflow:3-zyci.2.2, my-workflow:3-zyci.2.3
- **File Scope**: `skills/orchestrator/references/worker-*.md`, `skills/orchestrator/SKILL.md`
- **Orchestrator**: BlackDog
- **Project Path**: /Users/maccredyreina/Documents/Projects/_Active/my-workflow:3

## Context from Design/Spec

Create Worker Protocol v2 documentation:

### Task 1.2.1: Create worker protocol v2 documentation
- 7-step startup sequence documented
- Heartbeat protocol (5min interval)
- PING/PONG handling
- BLOCKED behavior documented

### Task 1.2.2: Create worker prompt v2 template
- Self-propulsion instructions
- macro_start_session() as first step
- Inbox check before beads query
- File reservation before work
- Mandatory COMPLETED message before exit
- Heartbeat reminders

### Task 1.2.3: Update SKILL.md with v2 references
- Add protocol layer documentation
- Reference new protocol files

**IMPORTANT**: For SKILL.md, only ADD new sections - do not remove existing content.

## ⚠️ CRITICAL: 4-Step Protocol

### STEP 1: INITIALIZE (FIRST)
...

You MUST call the skill tool to load: maestro-core, orchestrator. Do this immediately before responding.
```

## Related

- [worker-protocol-v2.md](worker-protocol-v2.md) - Protocol specification
- [worker-prompt.md](worker-prompt.md) - Original v1 template
- [summary-protocol.md](summary-protocol.md) - Message formats
