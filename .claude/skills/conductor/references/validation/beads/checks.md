# Beads Validation Checks

**Purpose:** Validate beads-related state files and detect issues.

---

## .fb-progress.json Schema Validation

### Required Fields

| Field | Type | Required | Default |
|-------|------|----------|---------|
| trackId | string | Yes | - |
| status | enum | Yes | "pending" |
| startedAt | ISO 8601 \| null | Yes | null |
| threadId | string \| null | No | null |
| resumeFrom | string \| null | No | null |
| epics | string[] | Yes | [] |
| issues | string[] | Yes | [] |
| planTasks | Record<string, string> | Yes | {} |
| beadToTask | Record<string, string> | Yes | {} |
| crossTrackDeps | string[] | No | [] |
| lastError | string \| null | No | null |
| lastVerified | ISO 8601 \| null | No | null |

### Status Values

```typescript
type FbProgressStatus = 'pending' | 'in_progress' | 'complete' | 'failed';
```

| Status | Description | Next Action |
|--------|-------------|-------------|
| pending | Beads not yet filed | Run `fb` to file |
| in_progress | Filing in progress | Wait or resume |
| complete | All beads filed | Ready for implementation |
| failed | Filing failed | Check lastError, retry |

---

## Validation Rules

### V-001: trackId Match

**Rule:** `trackId` must match directory name.

**Check:**
```
.fb-progress.json.trackId == parent directory name
```

**Auto-fix:** Update trackId to match directory name.

**Example:**
```
Directory: conductor/tracks/auth_20251225/
trackId should be: auth_20251225
```

---

### V-002: Status Validity

**Rule:** `status` must be one of: pending, in_progress, complete, failed.

**Check:**
```
status ∈ ['pending', 'in_progress', 'complete', 'failed']
```

**On Failure:** HALT - invalid status value.

---

### V-003: planTasks ↔ beadToTask Consistency

**Rule:** planTasks and beadToTask must be bidirectional inverses.

**Check:**
```
∀ (taskId, beadId) ∈ planTasks:
  beadToTask[beadId] == taskId
```

**On Failure:** HALT - mapping inconsistency.

**Example Error:**
```
planTasks["1.1.1"] = "bd-42"
beadToTask["bd-42"] = "1.1.2"  ← MISMATCH
```

---

### V-004: Beads Exist

**Rule:** All bead IDs in planTasks must exist in beads database.

**Check:**
```bash
for beadId in planTasks.values():
  bd show <beadId> --json  # Must succeed
```

**On Failure:** Warn - orphan mapping detected.

**Recovery:** Suggest `/conductor-migrate-beads` to reconcile.

---

### V-005: Plan Tasks Exist

**Rule:** All task IDs in planTasks must exist in plan.md.

**Check:**
```
∀ taskId ∈ planTasks.keys():
  taskId exists in plan.md task list
```

**On Failure:** Warn - stale mapping.

**Recovery:** Suggest updating planTasks or running `/conductor-revise`.

---

### V-006: Epic Linkage

**Rule:** All issues must be linked to their parent epic.

**Check:**
```bash
for epicId in epics:
  for issueId in issues:
    bd show <issueId> --json | jq '.dependents[] | select(.id == "<epicId>")'
```

**On Failure:** Warn - orphan issue.

**Recovery:** Link issue to epic: `bd dep add <issueId> <epicId>`.

---

### V-007: No Duplicate Mappings

**Rule:** Each plan task maps to exactly one bead.

**Check:**
```
len(planTasks) == len(set(planTasks.values()))
```

**On Failure:** HALT - duplicate bead assignments.

---

### V-008: Timestamps Valid

**Rule:** Timestamps must be valid ISO 8601 format.

**Check:**
```
startedAt, lastVerified → valid ISO 8601 or null
```

**On Failure:** Warn - invalid timestamp.

**Auto-fix:** Set to null if invalid.

---

## planTasks Mapping Validation

### Structure Check

```json
{
  "planTasks": {
    "1.1.1": "my-workflow:3-abc1",
    "1.1.2": "my-workflow:3-def2",
    "2.1.1": "my-workflow:3-ghi3"
  }
}
```

**Valid Task ID Formats:**
- Simple: `1.1`, `2.3`
- Hierarchical: `1.1.1`, `2.3.4`
- Prefixed: `task-1.1`, `T1.1`

**Valid Bead ID Formats:**
- Standard: `bd-42`, `bd-abc123`
- Prefixed: `my-workflow:3-abc1`

### Coverage Check

**Rule:** All plan.md tasks should have bead mappings.

**Check:**
```
tasks_in_plan = parse(plan.md).tasks
tasks_mapped = planTasks.keys()
missing = tasks_in_plan - tasks_mapped
```

**On Incomplete:**
```
⚠️ Plan tasks without beads:
  - 1.1.3: Update config file
  - 2.2.1: Add documentation

Run `fb` to file missing tasks, or `/conductor-revise` if plan changed.
```

---

## R/S/M Prompt for Malformed Plans

When plan.md parsing fails:

### Detection Criteria

| Issue | Description |
|-------|-------------|
| Missing task IDs | Tasks without `X.Y.Z` prefix |
| Invalid priority | Priority not in 0-4 range |
| Orphan dependencies | Depends on non-existent task |
| Circular dependencies | Task depends on itself |
| Duplicate IDs | Same task ID used twice |

### Prompt Format

```
Plan structure issue detected:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Issues found:
  • Missing task IDs on lines 45, 67
  • Invalid priority "high" (use 0-4) on line 89

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Choose action:
  [R]eformat - Auto-fix and continue
  [S]kip - Skip beads filing (plan-only mode)
  [M]anual - Abort for manual fix
```

### Auto-fix Rules (Reformat)

| Issue | Auto-fix |
|-------|----------|
| Missing task ID | Assign sequential ID |
| Invalid priority | Default to 2 (normal) |
| Orphan dependency | Remove dependency |
| Circular dependency | Remove circular edge |
| Duplicate ID | Append suffix (-2, -3) |

### --strict Flag (CI Mode)

When `--strict` is provided:
- No interactive prompt
- Any issue → HALT with exit code 1
- Output machine-readable error list

```bash
/conductor-newtrack auth_feature --strict
# Exit 1 with:
# {"errors": [{"line": 45, "type": "MISSING_TASK_ID", "message": "..."}]}
```

---

## Unsynced State Detection

### Check for Pending Operations

**Files to check:**
- `.conductor/pending_updates.jsonl`
- `.conductor/pending_closes.jsonl`
- `.conductor/unsynced.json`

**Preflight Check:**
```bash
if [ -f .conductor/pending_updates.jsonl ]; then
  echo "⚠️ Found pending updates - will replay"
fi
```

### Replay Protocol

1. Read pending operations file
2. For each operation:
   - Check idempotency key
   - If already applied → skip
   - If not applied → execute
3. On success → remove from pending file
4. On failure → increment retry count

**Idempotency Check:**
```bash
# For update status
current_status=$(bd show <id> --json | jq -r '.status')
if [ "$current_status" == "$target_status" ]; then
  echo "Already applied, skipping"
fi
```

---

## Session State Validation

### session-state_<agent>.json Check

| Field | Validation |
|-------|------------|
| agentId | Must match filename |
| mode | Must be "SA" or "MA" |
| modeLockedAt | Valid ISO 8601 |
| trackId | Must match active track or null |
| currentTask | Must exist in beads or null |
| tddPhase | Must be RED/GREEN/REFACTOR or null |
| lastUpdated | Valid ISO 8601, not future |

### Stale Agent Detection

**Rule:** Agent is stale if `lastUpdated` > 10 minutes ago.

**Check:**
```
now - session_state.lastUpdated > 10 minutes
```

**Action:**
- Warn user about stale session
- Offer to recover or start fresh

---

## Validation Output

### Summary Format

```
Beads Validation Summary
━━━━━━━━━━━━━━━━━━━━━━━━
Track: beads-integration_20251225

✓ .fb-progress.json schema valid
✓ planTasks ↔ beadToTask consistent
✓ All beads exist (31/31)
✓ All plan tasks mapped (31/31)
✓ Epic linkage valid

⚠️ Warnings:
  - 2 stale session files (auto-cleaned)

Status: PASS
```

### Error Format

```
Beads Validation FAILED
━━━━━━━━━━━━━━━━━━━━━━━━
Track: beads-integration_20251225

✗ V-003: planTasks ↔ beadToTask mismatch
  planTasks["1.1.1"] = "bd-42"
  beadToTask["bd-42"] = "1.1.2"

Action: Fix .fb-progress.json or run /conductor-migrate-beads
```

---

## References

- [Beads Facade](../../beads-facade.md) - Facade API
- [Beads Integration](../../beads-integration.md) - Integration points
- [Track Validation](../track/checks.md) - Track-level validation
