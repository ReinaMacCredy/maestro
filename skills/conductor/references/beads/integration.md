# Beads-Conductor Integration

**Version:** 1.0.0  
**Purpose:** Document all integration points between Conductor and Beads.

---

## Overview

Beads-Conductor integration achieves **zero manual bd commands** in the happy path by automating issue tracking throughout the Conductor workflow.

### Unified Architecture

All execution routes through orchestrator with Agent Mail coordination:

```
Session Start
     │
     ▼
┌─────────────┐
│  PREFLIGHT  │ ─── Check bd + Agent Mail availability
└─────────────┘
     │
     ▼
┌─────────────┐
│  REGISTER   │ ─── register_agent with Agent Mail
└─────────────┘
     │
     ▼
┌─────────────┐
│   EXECUTE   │ ─── bd CLI + file_reservation_paths
└─────────────┘
```

---

## Integration Points (13)

### Point 1: Preflight

**Trigger:** Session/command start  
**Conductor Command:** All  
**Beads Action:** Validate availability, register agent

**Behavior:**
1. Check `bd` availability → HALT if unavailable
2. Check Agent Mail availability → HALT if unavailable
3. Register agent with Agent Mail
4. Update metadata.json.session with session state
5. Recover pending operations

**HALT Conditions:**
- `bd` CLI not found
- `bd` CLI returns error on `bd version`
- Agent Mail unavailable

**Output:**
```
Preflight: bd v0.5.2 ✓, Agent Mail ✓
```

---

### Point 2: Init/Claim

**Trigger:** Starting task execution  
**Conductor Command:** `/conductor-implement`  
**Beads Action:** Claim task with Agent Mail coordination

**Also applicable to:** `/conductor-autonomous` (Ralph uses bd for story beads)

| Action | Command |
|--------|---------|
| Claim task | `bd update <taskId> --status in_progress` |
| Reserve files | `file_reservation_paths(paths=[...])` |

---

### Point 3: Reserve

**Trigger:** Before file edits  
**Conductor Command:** Task tool dispatch  
**Beads Action:** Lock files via Agent Mail

**Protocol:**
```python
file_reservation_paths(paths=["src/auth.ts"], ttl_seconds=600)  # 10 min TTL
# ... edit file ...
release_file_reservations()
```

**Conflict Resolution:**
1. If locked by another agent → check reservation status
2. Send message: `send_message(to=<agent>, subject="Need access to <file>")`
3. Check `fetch_inbox()` for response
4. Wait or pick different task

---

### Point 4-6: TDD Checkpoints (Default)

**Trigger:** Test phase transitions  
**Conductor Command:** `/conductor-implement` (TDD enabled by default)  
**Beads Action:** Checkpoint notes update

| Point | Phase | Notes Format |
|-------|-------|--------------|
| 4 | RED | `IN_PROGRESS: RED phase - writing failing test` |
| 5 | GREEN | `IN_PROGRESS: GREEN phase - making test pass` |
| 6 | REFACTOR | `IN_PROGRESS: REFACTOR phase - cleaning up code` |

**Skip Logic:**
- If no test files detected → skip checkpoints
- If `--no-tdd` flag provided → skip checkpoints

**metadata.json Session Update:**
```json
{
  "session": {
    "tdd_phase": "GREEN",
    "heartbeat": "2025-12-25T12:00:00Z"
  }
}
```

---

### Point 7: Close

**Trigger:** Task completion  
**Conductor Command:** `/conductor-implement`  
**Beads Action:** Complete task with reason

| Action | Command |
|--------|---------|
| Update notes | `bd update <taskId> --notes "..."` |
| Close task | `bd close <taskId> --reason <reason>` |
| Release files | `release_file_reservations()` |

**Close Reasons:**
- `completed` - Task finished successfully
- `skipped` - Task skipped (not needed)
- `blocked` - Task blocked, cannot proceed

**Notes Format:**
```
COMPLETED: <what was done>
KEY DECISION: <important choices>
NEXT: <follow-up if any>
```

---

### Point 8: Sync

**Trigger:** Session end, periodic  
**Conductor Command:** All (end)  
**Beads Action:** Push to git

**Behavior:**
1. Run `bd sync`
2. On failure: retry 3 times with backoff
3. On final failure: persist to `.conductor/unsynced.json`

**Retry Schedule:**
```
Attempt 1: Immediate
Attempt 2: Wait 1s
Attempt 3: Wait 2s
```

---

### Point 9: Compact

**Trigger:** Track finish  
**Conductor Command:** `/conductor-finish`  
**Beads Action:** Generate AI summaries

**Commands:**
```bash
bd compact --analyze --json      # Find issues needing summary
bd compact --apply --id <id> --summary "<text>"  # Apply summary
```

**Summary Format:**
```
COMPLETED: <concise summary of what was done>
IMPACT: <what this change enables>
```

---

### Point 10: Cleanup

**Trigger:** Track finish  
**Conductor Command:** `/conductor-finish`  
**Beads Action:** Remove old closed issues

**Threshold:** When closed issues > 150

**Commands:**
```bash
bd count --status closed --json      # Count closed
bd cleanup --older-than 0 --limit <n> --force  # Remove oldest
```

**Formula:** Remove `closed_count - 150` oldest issues

---

### Point 11: Track Init

**Trigger:** New track creation  
**Conductor Command:** `/conductor-newtrack`  
**Beads Action:** Create epic + issues from plan.md

**Flow:**
1. Parse plan.md tasks
2. Validate structure → R/S/M prompt if malformed
3. Create epic: `bd create "<title>" -t epic`
4. Create issues for each task
5. Wire dependencies between issues
6. Update `metadata.json.beads` section with planTasks mapping

**R/S/M Prompt (on malformed plan):**
```
Plan structure issue detected:
- Missing task IDs
- Invalid priority values

[R]eformat: Auto-fix and continue
[S]kip: Skip beads filing (plan-only mode)
[M]anual: Abort for manual fix
```

**`--strict` Flag (CI):**
- Fail immediately on malformed plan
- No interactive prompt

---

### Point 12: Status Sync

**Trigger:** Status check  
**Conductor Command:** `/conductor-status`  
**Beads Action:** Bidirectional status comparison

**Flow:**
1. Read Conductor state (plan.md, metadata.json)
2. Read Beads state (`bd list --json`)
3. Compare and detect discrepancies
4. Report and suggest reconciliation

**Discrepancy Types:**
| Type | Description | Suggestion |
|------|-------------|------------|
| Orphan bead | Bead not in planTasks | Delete or link |
| Missing bead | Plan task without bead | Create bead |
| Status mismatch | Plan says done, bead open | Close bead |

---

### Point 13: Revise/Reopen

**Trigger:** Spec/plan revision  
**Conductor Command:** `/conductor-revise`  
**Beads Action:** Reopen or create beads

**Flow:**
1. Identify affected plan items
2. Check if bead exists:
   - If exists and closed → reopen
   - If cleaned up → create new with lineage
   - If new plan item → create new
3. Update planTasks mapping

**Lineage for Cleaned-up Beads:**
```json
{
  "title": "Rework: Original task title",
  "notes": "Reopened from cleaned-up bd-42. Original closed 2025-12-20.",
  "metadata": {
    "originalBeadId": "bd-42",
    "reopenedAt": "2025-12-25T10:00:00Z",
    "reopenReason": "spec revision"
  }
}
```

---

## State Files

### Session State (metadata.json.session)

**Location:** `conductor/tracks/<track_id>/metadata.json`

Session tracking is stored in the metadata.json session section:

```json
{
  "session": {
    "updated": "2025-12-25T12:00:00Z",
    "session_id": "T-abc123",
    "platform": "amp",
    "bound_bead": "bd-42",
    "tdd_phase": "GREEN",
    "heartbeat": "2025-12-25T12:00:00Z"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| bound_bead | string \| null | Claimed task ID |
| tdd_phase | "RED" \| "GREEN" \| "REFACTOR" \| null | TDD phase |
| heartbeat | ISO 8601 | Last activity timestamp |

---

### Beads State (metadata.json.beads)

**Location:** `tracks/<track-id>/metadata.json`

```json
{
  "beads": {
    "status": "complete",
    "startedAt": "2025-12-25T19:39:00Z",
    "epicId": "my-workflow:3-1w8y",
    "epics": [{"id": "my-workflow:3-1w8y", "title": "...", "status": "complete", "createdAt": "...", "reviewed": true}],
    "issues": ["my-workflow:3-51f9", "my-workflow:3-kt2n"],
    "planTasks": {
      "1.1.1": "my-workflow:3-51f9",
      "1.1.2": "my-workflow:3-kt2n"
    },
    "beadToTask": {
      "my-workflow:3-51f9": "1.1.1",
      "my-workflow:3-kt2n": "1.1.2"
    },
    "crossTrackDeps": [],
    "reviewStatus": "complete",
    "reviewedAt": "2025-12-25T12:00:00Z"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| status | "pending" \| "in_progress" \| "complete" \| "failed" | Filing status |
| planTasks | Record<string, string> | Plan ID → Bead ID |
| beadToTask | Record<string, string> | Bead ID → Plan ID (reverse) |
| reviewedAt | ISO 8601 | Last review timestamp |

---

### Session Lock (`session-lock_<track-id>.json`)

**Location:** `.conductor/`

Prevents concurrent SA sessions on same track.

```json
{
  "agentId": "T-abc123",
  "lockedAt": "2025-12-25T10:00:00Z",
  "lastHeartbeat": "2025-12-25T10:25:00Z",
  "pid": 12345
}
```

**Heartbeat Protocol:**
- Active sessions update every 5 minutes
- Stale detection: heartbeat > 10 min ago

**Conflict Resolution:**
```
Another session is active on this track.
[C]ontinue anyway (risk conflicts)
[W]ait for other session
[F]orce unlock (other session will error)
```

---

### Pending Operations

**Location:** `.conductor/`

Operations that failed after retries, pending replay.

**pending_updates.jsonl:**
```jsonl
{"op": "update", "id": "bd-42", "idempotencyKey": "T-abc_1703509200_update_bd-42", "args": ["--status", "in_progress"], "ts": "2025-12-25T10:00:00Z", "retries": 3}
```

**pending_closes.jsonl:**
```jsonl
{"op": "close", "id": "bd-43", "idempotencyKey": "T-abc_1703509260_close_bd-43", "args": ["--reason", "completed"], "ts": "2025-12-25T10:01:00Z", "retries": 3}
```

**Recovery:** On next session, preflight replays pending operations.

---

## Subagent Rules

When Conductor dispatches subagents via Task tool:

### Allowed (Read-Only)
```bash
bd show <id> --json
bd ready --json
bd list --json
```

### Blocked (Write Operations)
```bash
bd update   # Return to main agent
bd close    # Return to main agent
bd create   # Return to main agent
```

**Why:** Main agent centralizes writes to prevent race conditions and ensure consistent state.

**Subagent Return Format:**
```json
{
  "status": "success",
  "beadUpdates": [
    { "id": "bd-42", "action": "close", "reason": "completed", "notes": "..." }
  ]
}
```

Main agent processes `beadUpdates` after subagent returns.

---

## Session Flow

All execution uses Agent Mail coordination:

```
Preflight
    │
    ▼
register_agent(project, program, model)
    │
    ▼
bd ready --json ──► Select task
    │
    ▼
bd update <id> --status in_progress
    │
    ▼
file_reservation_paths(paths=[...]) ──► Reserve files
    │
    ▼
Work on task (with TDD checkpoints by default)
    │
    ▼
bd close <id> --reason completed
    │
    ▼
release_file_reservations()
    │
    ▼
send_message(summary) ──► Notify coordinator
    │
    ▼
bd sync
```

---

## HALT Conditions

| Condition | Action |
|-----------|--------|
| bd unavailable | HALT |
| Agent Mail unavailable | HALT |
| bd fails mid-session | Retry 3x → persist → warn |

---

## Output Format Specification

All beads operations output JSON for parsing:

```bash
# Ready tasks
bd ready --json
# → [{"id": "bd-42", "title": "...", "priority": 0, ...}]

# Show task
bd show bd-42 --json
# → [{"id": "bd-42", "title": "...", "notes": "...", "dependents": [...]}]

# List tasks
bd list --status open --json
# → [{"id": "...", ...}, ...]
```

**Filtering:**
```bash
bd list --status open --json | jq '.[] | select(.priority <= 1)'
```

---

## References

- [Beads Facade](facade.md) - Facade API contract
- [Beads Validation](../validation/beads-checks.md) - State file validation
- [Design Document](../../../conductor/archive/beads-conductor-integration_20251225/design.md)
- → Load [tracking skill](../../../tracking/SKILL.md) for full beads workflow
