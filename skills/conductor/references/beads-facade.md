# Beads Facade Contract

**Version:** 1.0.0  
**Purpose:** Single integration layer between Conductor and Beads.

---

## Overview

The Beads Facade provides a unified interface for Conductor workflows to interact with Beads issue tracking. It abstracts the difference between:

- **SA Mode (Single-Agent):** Direct `bd` CLI calls
- **MA Mode (Multi-Agent):** Village MCP server calls

All Conductor components use this facade instead of calling `bd` directly.

---

## Interface

### checkAvailability()

Checks if Beads is available and returns version info.

**Returns:**
```typescript
{
  available: boolean;      // true if bd CLI responds
  version?: string;        // e.g., "0.5.2"
  error?: string;          // Error message if unavailable
}
```

**Behavior:**
- Runs `bd version` with 2s timeout
- On timeout: returns `{ available: false, error: "BD_TIMEOUT" }`
- On command not found: returns `{ available: false, error: "BD_UNAVAILABLE" }`

**Example:**
```json
// Success
{ "available": true, "version": "0.5.2" }

// Failure  
{ "available": false, "error": "BD_UNAVAILABLE: bd command not found" }
```

---

### detectMode()

Determines whether to use SA or MA mode.

**Returns:**
```typescript
{
  mode: 'SA' | 'MA';
  reason: string;
  villageAvailable?: boolean;
}
```

**Mode Selection Precedence:**
1. Existing session-state file → use locked mode
2. User preference (`preferences.json`) → use preferred mode
3. Village available + no preference → MA
4. Fallback → SA

**Example:**
```json
{ "mode": "SA", "reason": "Village MCP unavailable" }
{ "mode": "MA", "reason": "Village available, user preference" }
```

---

### createEpicFromPlan()

Creates an epic and issues from a plan.md file.

**Input:**
```typescript
{
  trackId: string;          // e.g., "beads-integration_20251225"
  planPath: string;         // Absolute path to plan.md
  epicTitle: string;        // Title for the epic
  tasks: Array<{
    id: string;             // Plan task ID (e.g., "1.1.1")
    title: string;          // Task title
    priority: 0|1|2|3|4;    // P0=critical, P4=backlog
    depends?: string[];     // Task IDs this depends on
  }>;
}
```

**Returns:**
```typescript
{
  epicId: string;                           // Created epic bead ID
  taskIds: string[];                        // Created issue bead IDs
  planTasksMapping: Record<string, string>; // { "1.1.1": "bd-42", ... }
}
```

**Behavior:**
1. Creates epic: `bd create "<epicTitle>" -t epic -p 0`
2. For each task:
   - Creates issue: `bd create "<title>" -t task -p <priority>`
   - Links to epic: `bd dep add <issueId> <epicId>`
   - Links dependencies: `bd dep add <issueId> <depIssueId>`
3. Updates `metadata.json.beads` section with planTasks mapping

**Error Codes:**
- `EPIC_EXISTS`: Epic with same title already exists
- `PARSE_ERROR`: plan.md parsing failed
- `BD_UNAVAILABLE`: bd CLI not available

**Example:**
```json
// Input
{
  "trackId": "auth_20251225",
  "planPath": "/path/to/plan.md",
  "epicTitle": "Epic: User Authentication",
  "tasks": [
    { "id": "1.1", "title": "Create login endpoint", "priority": 0 },
    { "id": "1.2", "title": "Add JWT tokens", "priority": 0, "depends": ["1.1"] }
  ]
}

// Output
{
  "epicId": "my-workflow:3-abc1",
  "taskIds": ["my-workflow:3-def2", "my-workflow:3-ghi3"],
  "planTasksMapping": {
    "1.1": "my-workflow:3-def2",
    "1.2": "my-workflow:3-ghi3"
  }
}
```

---

### claimTask()

Claims a task for the current session.

**Input:**
```typescript
{
  taskId: string;           // Bead ID to claim
  mode: 'SA' | 'MA';        // Current session mode
}
```

**Returns:**
```typescript
{
  success: boolean;
  alreadyClaimed?: boolean; // true if another agent has it
  claimedBy?: string;       // Agent ID if already claimed
}
```

**Behavior by Mode:**

| Mode | Action |
|------|--------|
| SA | `bd update <taskId> --status in_progress` |
| MA | Village `claim(<taskId>)` (atomic) |

**Race Condition Handling (MA):**
- First claim wins (by `modeLockedAt` timestamp)
- Tie-breaker: lexicographic agent ID

**Error Codes:**
- `CLAIM_CONFLICT`: Task claimed by another agent
- `BD_TIMEOUT`: Command timed out

**Example:**
```json
// Success
{ "success": true }

// Conflict
{ "success": false, "alreadyClaimed": true, "claimedBy": "T-def456" }
```

---

### closeTask()

Closes a task with a reason.

**Input:**
```typescript
{
  taskId: string;
  reason: 'completed' | 'skipped' | 'blocked';
  notes?: string;           // Optional completion notes
}
```

**Returns:**
```typescript
{
  success: boolean;
  error?: string;
}
```

**Behavior:**
```bash
# SA Mode
bd close <taskId> --reason "<reason>"
bd update <taskId> --notes "<notes>"

# MA Mode
done(<taskId>, reason="<reason>")  # Auto-releases reservations
```

**Retry Logic:**
- Retries 3 times on failure
- On final failure: persists to `.conductor/pending_closes.jsonl`

**Example:**
```json
// Input
{ "taskId": "bd-42", "reason": "completed", "notes": "COMPLETED: Auth flow working" }

// Output
{ "success": true }
```

---

### updateTddPhase()

Updates the TDD phase for a task (enabled by default, disable with `--no-tdd`).

**Input:**
```typescript
{
  taskId: string;
  phase: 'RED' | 'GREEN' | 'REFACTOR';
}
```

**Returns:**
```typescript
{
  success: boolean;
}
```

**Behavior:**
- Updates LEDGER.md frontmatter: `tdd_phase: <phase>`
- Updates bead notes: `IN_PROGRESS: <phase> phase`

**Notes Format:**
```text
IN_PROGRESS: RED phase - writing failing test
IN_PROGRESS: GREEN phase - making test pass
IN_PROGRESS: REFACTOR phase - cleaning up code
```

**Example:**
```json
// Input
{ "taskId": "bd-42", "phase": "GREEN" }

// Output
{ "success": true }
```

---

### syncToGit()

Syncs beads state to git.

**Input:**
```typescript
{
  retries?: number;         // Default: 3
}
```

**Returns:**
```typescript
{
  success: boolean;
  synced: number;           // Number of changes synced
  unsynced?: string[];      // Bead IDs that failed to sync
}
```

**Behavior:**
1. Runs `bd sync`
2. On failure: retries up to N times (default 3)
3. On final failure: persists unsynced state to `.conductor/unsynced.json`

**Example:**
```json
// Success
{ "success": true, "synced": 5 }

// Partial failure
{ "success": false, "synced": 3, "unsynced": ["bd-42", "bd-43"] }
```

---

## Error Types

All facade methods may return these error types:

```typescript
type FacadeError = {
  code: ErrorCode;
  message: string;
  recoverable: boolean;
}

type ErrorCode =
  | 'BD_UNAVAILABLE'    // bd CLI not found
  | 'BD_TIMEOUT'        // Command timed out
  | 'SYNC_FAILED'       // Git sync failed after retries
  | 'EPIC_EXISTS'       // Epic already exists
  | 'PARSE_ERROR'       // plan.md parsing failed
  | 'CLAIM_CONFLICT';   // Task claimed by another agent
```

**Recoverability:**

| Error | Recoverable | Recovery Action |
|-------|-------------|-----------------|
| BD_UNAVAILABLE | No | HALT - install bd |
| BD_TIMEOUT | Yes | Retry with backoff |
| SYNC_FAILED | Yes | Persist for later retry |
| EPIC_EXISTS | No | Use existing epic |
| PARSE_ERROR | No | Fix plan.md format |
| CLAIM_CONFLICT | Yes | Pick different task |

---

## Retry Logic

Standard retry behavior for transient failures:

```text
Attempt 1: Immediate
Attempt 2: Wait 1s
Attempt 3: Wait 2s
```

**Pending Operations File:**

Failed operations are persisted for later replay:

```jsonl
{"op": "update", "id": "bd-42", "idempotencyKey": "T-abc_1703509200_update_bd-42", "args": ["--status", "in_progress"], "ts": "2025-12-25T10:00:00Z", "retries": 3}
{"op": "close", "id": "bd-43", "idempotencyKey": "T-abc_1703509260_close_bd-43", "args": ["--reason", "completed"], "ts": "2025-12-25T10:01:00Z", "retries": 3}
```

**Idempotency:**
- Key format: `<agent-id>_<unix-timestamp>_<operation>_<bead-id>`
- Before replay: check if operation already applied
- `bd update --status X` is idempotent (same status = no-op)
- `bd close` is idempotent (already closed = no-op)
- `bd create` is NOT idempotent (would create duplicate) → HALT on failure

---

## State Files

The facade reads/writes these state files:

| File | Location | Purpose |
|------|----------|---------|
| `LEDGER.md` | `conductor/sessions/active/` | Session tracking (mode, tdd_phase, bound_track) |
| `metadata.json.beads` | `tracks/<id>/` | planTasks mapping, filing status |
| `pending_updates.jsonl` | `.conductor/` | Failed update operations |
| `pending_closes.jsonl` | `.conductor/` | Failed close operations |
| `unsynced.json` | `.conductor/` | Unsynced bead IDs |

---

## Usage in Conductor

```markdown
# Example: Preflight
1. Call checkAvailability()
2. If unavailable → HALT
3. Call detectMode()
4. Lock mode in session-state

# Example: Track Init
1. Parse plan.md tasks
2. Call createEpicFromPlan()
3. Update metadata.json.beads with mapping

# Example: Task Execution
1. Call claimTask(taskId, mode)
2. Work on task
3. Unless --no-tdd: call updateTddPhase() at each phase
4. Call closeTask(taskId, reason)
5. Call syncToGit() at session end
```

---

## References

- [Beads Integration Points](beads-integration.md)
- [Beads Validation](validation/beads/checks.md)
- [Design Document](../../../conductor/archive/beads-conductor-integration_20251225/design.md)
