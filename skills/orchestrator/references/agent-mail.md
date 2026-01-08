# Agent Mail Protocol

## Orchestrator Registration (Phase 2)

On spawn, the orchestrator MUST:
1. Register itself via `macro_start_session`
2. Workers self-register via `macro_start_session` on startup

```python
# Orchestrator initialization
macro_start_session(
    human_key="/path/to/project",
    program="amp",
    model="claude-sonnet-4-20250514",
    task_description=f"Orchestrator for epic {epic_id}"
)

# Workers self-register on startup (no pre-registration needed)
# Each worker calls macro_start_session which handles:
# - ensure_project (idempotent)
# - register_agent (auto-generates name)
# - file_reservation_paths (optional)
# - fetch_inbox (returns prior context)
```

> **Why self-register?** Workers calling `macro_start_session` handles all setup atomically. The orchestrator only needs to register itself—workers register on spawn. Use `auto_contact_if_blocked=True` on `send_message` to auto-establish contact with newly-registered workers.

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
