# Agent Mail Protocol

## Orchestrator Registration (Phase 2)

On spawn, the orchestrator MUST:
1. Register itself
2. **Pre-register ALL workers** before spawning them

```python
# 1. Ensure project exists
ensure_project(human_key="/path/to/project")

# 2. Register orchestrator identity
register_agent(
    project_key="/path/to/project",
    name="OrchestratorName",  # Auto-generated adjective+noun
    program="amp",
    model="claude-sonnet-4-20250514",
    task_description=f"Orchestrator for epic {epic_id}"
)

# 3. Pre-register ALL workers (CRITICAL - do this BEFORE spawning)
for track in TRACKS:
    register_agent(
        project_key="/path/to/project",
        name=track.agent,  # e.g., "BlueStar", "GreenMountain"
        program="amp",
        model="claude-sonnet-4-20250514",
        task_description=f"Worker for Track {track.track}"
    )

# Now send_message to workers will succeed
```

> **Why pre-register?** `send_message` validates recipients exist. Without pre-registration, messaging workers fails with "recipients not registered" error.

## Inbox Fetch Pattern

Check inbox for context from prior sessions and worker updates:

```python
# On session start - load prior context
messages = fetch_inbox(
    project_key="/path/to/project",
    agent_name="OrchestratorName",
    include_bodies=True,
    limit=20
)

# Process prior context
for msg in messages:
    if "[TRACK COMPLETE]" in msg.subject:
        mark_track_complete(msg)
    elif "[BLOCKER]" in msg.subject:
        handle_blocker(msg)
    elif "[HEARTBEAT]" in msg.subject:
        update_worker_status(msg)
```

## Mandatory Summary Protocol

All workers (including orchestrator) MUST send a summary before returning:

```python
send_message(
    project_key="/path/to/project",
    sender_name="OrchestratorName",
    to=all_workers,
    thread_id=epic_id,
    subject="EPIC COMPLETE: {title}",
    body_md="""
## Status
SUCCEEDED

## Files Changed
- path/to/file.ts (added)

## Key Decisions
- Decision: rationale

## Issues (if any)
None
"""
)
```

See [summary-protocol.md](summary-protocol.md) for complete format.

## Session Brain (Phase 0)

The orchestrator includes a "session brain" that coordinates multiple Amp sessions:

- **Auto-registration**: Sessions register identity with Agent Mail on startup
- **Conflict detection**: Warns when sessions work on same track/files/beads
- **Stale takeover**: Prompts to take over inactive sessions (>10 min)
- **Always-on**: Preflight runs automatically on /conductor-implement and /conductor-orchestrate

### Session Identity Format

- Internal: `{BaseAgent}-{timestamp}` (unique, e.g., `BlueLake-1735689600`)
- Display: `{BaseAgent} (session HH:MM)` (human-readable, e.g., `BlueLake (session 10:30)`)

See [preflight.md](preflight.md) for protocol details.
