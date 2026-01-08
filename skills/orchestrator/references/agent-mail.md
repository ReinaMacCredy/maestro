# Agent Mail Protocol

> **Note:** All examples use CLI syntax (`--arg value`). MCP function calls are available as fallback but CLI is preferred for consistency.

## Orchestrator Registration (Phase 2)

On spawn, the orchestrator MUST:
1. Register itself via `macro-start-session` CLI
2. Workers self-register via `macro-start-session` on startup

```bash
# Orchestrator initialization
toolboxes/agent-mail/agent-mail.js macro-start-session \
  --human-key /path/to/project \
  --program amp \
  --model claude-opus-4-5@20251101 \
  --task-description "Orchestrator for epic ${epic_id}"

# Workers self-register on startup (no pre-registration needed)
# Each worker calls macro-start-session which handles:
# - ensure_project (idempotent)
# - register_agent (auto-generates name)
# - file_reservation_paths (optional)
# - fetch_inbox (returns prior context)
```

> **Why self-register?** Workers calling `macro-start-session` handles all setup atomically. The orchestrator only needs to register itself—workers register on spawn. Use `--auto-contact-if-blocked true` on `send-message` to auto-establish contact with newly-registered workers.

## Inbox Fetch Pattern

Check inbox for context from prior sessions and worker updates:

```bash
# On session start - load prior context
toolboxes/agent-mail/agent-mail.js fetch-inbox \
  --project-key /path/to/project \
  --agent-name OrchestratorName \
  --include-bodies true \
  --limit 20
```

```python
# Parse JSON output and process prior context
import json
messages = json.loads(output)

for msg in messages:
    if "[TRACK COMPLETE]" in msg.get("subject", ""):
        mark_track_complete(msg)
    elif "[BLOCKER]" in msg.get("subject", ""):
        handle_blocker(msg)
    elif "[HEARTBEAT]" in msg.get("subject", ""):
        update_worker_status(msg)
```

## Mandatory Summary Protocol

All workers (including orchestrator) MUST send a summary before returning:

```bash
toolboxes/agent-mail/agent-mail.js send-message \
  --project-key /path/to/project \
  --sender-name OrchestratorName \
  --to '["Worker1", "Worker2"]' \
  --thread-id ${epic_id} \
  --subject "EPIC COMPLETE: ${title}" \
  --body-md "## Status
SUCCEEDED

## Files Changed
- path/to/file.ts (added)

## Key Decisions
- Decision: rationale

## Issues (if any)
None"
```

See [summary-protocol.md](summary-protocol.md) for complete format.

## Session Brain (Phase 0)

The orchestrator includes a "session brain" that coordinates multiple Amp sessions:

- **Auto-registration**: Sessions register identity with Agent Mail CLI on startup
- **Conflict detection**: Warns when sessions work on same track/files/beads
- **Stale takeover**: Prompts to take over inactive sessions (>10 min)
- **Always-on**: Preflight runs automatically on /conductor-implement and /conductor-orchestrate

### Session Identity Format

- Internal: `{BaseAgent}-{timestamp}` (unique, e.g., `BlueLake-1735689600`)
- Display: `{BaseAgent} (session HH:MM)` (human-readable, e.g., `BlueLake (session 10:30)`)

See [preflight.md](preflight.md) for protocol details.
