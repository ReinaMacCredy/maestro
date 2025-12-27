# Track Validation Checks

Core validation logic for Conductor tracks. Inline this file in command Phase 0.

## Validation Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Phase 0: Track Validation                    │
├─────────────────────────────────────────────────────────────────────┤
│ 0.1 Resolve track path + set DIAGNOSE_MODE                          │
│ 0.2 Check directory exists and not empty                            │
│ 0.3 File existence matrix (with collective HAS_STATE)               │
│ 0.4 Validate JSON files (parseability + required fields)            │
│ 0.5 Auto-create missing state files (if conditions met)             │
│ 0.6 Auto-fix track_id mismatches in state files                     │
│ 0.7 Staleness detection (in_progress, failed, stale complete)       │
└─────────────────────────────────────────────────────────────────────┘
```

## Step 0.1: Resolve Track Path

```bash
TRACK_ID="<provided_track_id>"
TRACK_DIR="conductor/tracks/${TRACK_ID}"

# Parse --diagnose flag (report only, no modifications)
DIAGNOSE_MODE=0
[[ "$1" == "--diagnose" || "$2" == "--diagnose" ]] && DIAGNOSE_MODE=1

# Parse --allow-stale flag (convert staleness halts to warnings, for archiving)
# Note: Use ALLOW_STALE_FLAG to avoid shadowing the variable being set
ALLOW_STALE=0
[[ "$1" == "--allow-stale" || "$2" == "--allow-stale" ]] && ALLOW_STALE=1

if [[ ! -d "$TRACK_DIR" ]]; then
  TRACK_DIR="conductor/archive/${TRACK_ID}"
fi

if [[ ! -d "$TRACK_DIR" ]]; then
  echo "HALT: Track not found: $TRACK_ID"
  exit 1
fi

# Re-derive track_id from directory to ensure consistency
TRACK_ID=$(basename "$TRACK_DIR")
```

## Step 0.2: Check Directory

```bash
if [[ -z "$(ls -A "$TRACK_DIR" 2>/dev/null)" ]]; then
  echo "WARN: Empty track directory: $TRACK_ID"
  echo "SKIP: No files to validate"
  exit 0  # SKIP, not HALT
fi
```

## Step 0.3: File Existence Matrix

Check file combinations and determine action.

**State File:** `metadata.json` - Contains all track state including `generation` and `beads` sections.

| design.md | spec.md | plan.md | metadata.json | Action                     |
| --------- | ------- | ------- | ------------- | -------------------------- |
| ✗         | ✗       | ✗       | ✗             | SKIP + warn (empty)        |
| ✓         | ✗       | ✗       | ✗             | PASS (design-only state)   |
| ✗         | ✓       | ✗       | ✗             | HALT (spec without plan)   |
| ✗         | ✗       | ✓       | ✗             | HALT (plan without spec)   |
| ✗/✓       | ✓       | ✓       | ✗             | Auto-create metadata.json  |
| ✗/✓       | ✓       | ✓       | ✓             | Validate and PASS          |

**See also:** [Beads Validation](../beads/checks.md) for `metadata.json.beads` schema validation and planTasks mapping checks.

```bash
HAS_DESIGN=$([[ -f "$TRACK_DIR/design.md" ]] && echo 1 || echo 0)
HAS_SPEC=$([[ -f "$TRACK_DIR/spec.md" ]] && echo 1 || echo 0)
HAS_PLAN=$([[ -f "$TRACK_DIR/plan.md" ]] && echo 1 || echo 0)
HAS_METADATA=$([[ -f "$TRACK_DIR/metadata.json" ]] && echo 1 || echo 0)

# Check XOR condition (invalid state)
if [[ $HAS_SPEC -eq 1 && $HAS_PLAN -eq 0 ]]; then
  echo "HALT: spec.md exists without plan.md in $TRACK_ID"
  exit 1
fi
if [[ $HAS_SPEC -eq 0 && $HAS_PLAN -eq 1 ]]; then
  echo "HALT: plan.md exists without spec.md in $TRACK_ID"
  exit 1
fi

# Design-only is valid
if [[ $HAS_DESIGN -eq 1 && $HAS_SPEC -eq 0 && $HAS_PLAN -eq 0 ]]; then
  echo "PASS: Design-only track $TRACK_ID"
  exit 0
fi
```

## Step 0.4: Validate JSON Files

All JSON files must be parseable. HALT on corruption.

**Scope:** This step validates parseability and required fields only. Full JSON schema validation (against `skills/conductor/references/schemas/*.json`) is the responsibility of higher-level tooling like `/conductor-validate`.

```bash
for json_file in "$TRACK_DIR"/*.json; do
  [[ -f "$json_file" ]] || continue

  # Check parseability
  if ! jq empty "$json_file" 2>/dev/null; then
    echo "HALT: Corrupted JSON: $json_file"
    exit 1
  fi

  # Check required fields for metadata.json
  BASENAME=$(basename "$json_file")
  if [[ "$BASENAME" == "metadata.json" ]]; then
    if [[ -z "$(jq -r '.track_id // empty' "$json_file")" ]]; then
      echo "WARN: metadata.json missing track_id field"
    fi
    if [[ -z "$(jq -r '.status // empty' "$json_file")" ]]; then
      echo "WARN: metadata.json missing status field"
    fi
    # Optional: check generation and beads sections exist
    if [[ "$(jq 'has("generation")' "$json_file")" != "true" ]]; then
      echo "WARN: metadata.json missing generation section"
    fi
    if [[ "$(jq 'has("beads")' "$json_file")" != "true" ]]; then
      echo "WARN: metadata.json missing beads section"
    fi
  fi
done
```

## Step 0.5: Auto-Create Missing metadata.json

When spec.md + plan.md exist but metadata.json is missing:

**Pre-checks (all must pass):**

1. Both files have content (size > 0)
2. Both files < 30 days old
3. No track_id mismatch in headers (if headers exist) - check BOTH files

```bash
# Trigger: spec+plan exist but metadata.json is missing
if [[ $HAS_SPEC -eq 1 && $HAS_PLAN -eq 1 && $HAS_METADATA -eq 0 ]]; then
  # Pre-check 1: Both files have content
  if [[ ! -s "$TRACK_DIR/spec.md" || ! -s "$TRACK_DIR/plan.md" ]]; then
    echo "HALT: Empty spec.md or plan.md in $TRACK_ID"
    exit 1
  fi

  # Pre-check 2: BOTH files < 30 days old
  THIRTY_DAYS_AGO=$(date -v-30d +%s 2>/dev/null || date -d '30 days ago' +%s)

  SPEC_MTIME=$(stat -f %m "$TRACK_DIR/spec.md" 2>/dev/null || stat -c %Y "$TRACK_DIR/spec.md")
  if [[ $SPEC_MTIME -lt $THIRTY_DAYS_AGO ]]; then
    if [[ $ALLOW_STALE -eq 1 ]]; then
      echo "WARN: spec.md older than 30 days (continuing with --allow-stale)"
    else
      echo "HALT: spec.md older than 30 days, manual review required"
      exit 1
    fi
  fi

  PLAN_MTIME=$(stat -f %m "$TRACK_DIR/plan.md" 2>/dev/null || stat -c %Y "$TRACK_DIR/plan.md")
  if [[ $PLAN_MTIME -lt $THIRTY_DAYS_AGO ]]; then
    if [[ $ALLOW_STALE -eq 1 ]]; then
      echo "WARN: plan.md older than 30 days (continuing with --allow-stale)"
    else
      echo "HALT: plan.md older than 30 days, manual review required"
      exit 1
    fi
  fi

  # Pre-check 3: Header track_id (if present) matches directory - check BOTH files
  for content_file in spec.md plan.md; do
    HEADER_ID=$(grep -m1 '^# .*Track:' "$TRACK_DIR/$content_file" | sed 's/.*Track: *//' | tr -d ' ')
    if [[ -n "$HEADER_ID" && "$HEADER_ID" != "$TRACK_ID" ]]; then
      echo "WARN: Header track_id mismatch in $content_file: $HEADER_ID vs $TRACK_ID"
      echo "HALT: Manual review required for track_id mismatch in content files"
      exit 1
    fi
  done

  # All pre-checks passed
  if [[ $DIAGNOSE_MODE -eq 1 ]]; then
    echo "DIAGNOSE: Would auto-create metadata.json for $TRACK_ID"
  else
    echo "Auto-creating metadata.json for $TRACK_ID"
    # See snippets.md for auto_create_metadata function
  fi
fi
```

## Step 0.6: Auto-Fix track_id Mismatches

Directory name is source of truth. Auto-fix mismatches in metadata.json.

```bash
# Fix metadata.json
if [[ -f "$TRACK_DIR/metadata.json" ]]; then
  CURRENT_ID=$(jq -r '.track_id // empty' "$TRACK_DIR/metadata.json")
  if [[ -n "$CURRENT_ID" && "$CURRENT_ID" != "$TRACK_ID" ]]; then
    if [[ $DIAGNOSE_MODE -eq 1 ]]; then
      echo "DIAGNOSE: Would fix track_id in metadata.json: $CURRENT_ID → $TRACK_ID"
    else
      echo "Auto-fixing track_id in metadata.json: $CURRENT_ID → $TRACK_ID"
      jq --arg id "$TRACK_ID" '.track_id = $id' "$TRACK_DIR/metadata.json" > "$TRACK_DIR/metadata.json.tmp.$$"
      mv "$TRACK_DIR/metadata.json.tmp.$$" "$TRACK_DIR/metadata.json"
      # Log repair (see snippets.md for audit trail)
    fi
  fi
fi
```

**Warn (don't auto-fix) for content files:**

```bash
for content_file in design.md spec.md plan.md; do
  [[ -f "$TRACK_DIR/$content_file" ]] || continue
  HEADER_ID=$(grep -m1 '^# .*Track:' "$TRACK_DIR/$content_file" | sed 's/.*Track: *//' | tr -d ' ')
  if [[ -n "$HEADER_ID" && "$HEADER_ID" != "$TRACK_ID" ]]; then
    echo "WARN: Header track_id mismatch in $content_file: $HEADER_ID vs $TRACK_ID"
  fi
done
```

## Step 0.7: Staleness Detection

Detect three types of staleness: `in_progress`, `failed`, and stale `complete` in metadata.json.beads.

```bash
if [[ -f "$TRACK_DIR/metadata.json" ]]; then
  BEADS_STATUS=$(jq -r '.beads.status // empty' "$TRACK_DIR/metadata.json")

  # Case 1: Stale in_progress
  if [[ "$BEADS_STATUS" == "in_progress" ]]; then
    # Get thread from workflow history
    LAST_THREAD=$(jq -r '.threads[-1].id // "unknown"' "$TRACK_DIR/metadata.json")
    UPDATED_AT=$(jq -r '.updated_at // "unknown"' "$TRACK_DIR/metadata.json")
    echo ""
    echo "⚠️  STALE IN_PROGRESS DETECTED"
    echo "   Track: $TRACK_ID"
    echo "   Updated: $UPDATED_AT"
    echo "   Thread: $LAST_THREAD"
    echo ""
    echo "Options:"
    echo "  [R]esume - Continue from last checkpoint"
    echo "  [X]Reset - Clear progress and start fresh"
    echo "  [D]iagnose - Show detailed state"
    echo ""
    # Never auto-reset - require explicit user action
  fi

  # Case 2: Failed status
  if [[ "$BEADS_STATUS" == "failed" ]]; then
    UPDATED_AT=$(jq -r '.updated_at // "unknown"' "$TRACK_DIR/metadata.json")
    echo ""
    echo "⚠️  FAILED STATUS DETECTED"
    echo "   Track: $TRACK_ID"
    echo "   Updated: $UPDATED_AT"
    echo ""
    echo "Options:"
    echo "  [R]esume - Retry from last checkpoint (fb $TRACK_ID)"
    echo "  [X]Reset - Clear progress and start fresh (fb $TRACK_ID --force)"
    echo "  [D]iagnose - Show detailed state (/conductor-validate $TRACK_ID --diagnose)"
    echo ""
  fi

  # Case 3: Stale complete (reviewedAt > 7 days old)
  if [[ "$BEADS_STATUS" == "complete" ]]; then
    REVIEWED_AT=$(jq -r '.beads.reviewedAt // empty' "$TRACK_DIR/metadata.json")
    if [[ -n "$REVIEWED_AT" ]]; then
      # Convert reviewedAt to epoch
      VERIFIED_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$REVIEWED_AT" +%s 2>/dev/null || \
                       date -d "$REVIEWED_AT" +%s 2>/dev/null || echo 0)
      SEVEN_DAYS_AGO=$(date -v-7d +%s 2>/dev/null || date -d '7 days ago' +%s)

      if [[ $VERIFIED_EPOCH -lt $SEVEN_DAYS_AGO && $VERIFIED_EPOCH -gt 0 ]]; then
        echo ""
        echo "ℹ️  STALE COMPLETE STATUS"
        echo "   Track: $TRACK_ID"
        echo "   Last reviewed: $REVIEWED_AT"
        echo ""
        echo "   Consider running 'rb $TRACK_ID' to re-verify beads."
        echo ""
      fi
    fi
  fi
fi
```

## Diagnose Mode

When `--diagnose` flag is set, report all issues without making changes:

```bash
if [[ $DIAGNOSE_MODE -eq 1 ]]; then
  echo "=== Track Validation Report: $TRACK_ID ==="
  echo ""
  echo "Files:"
  ls -la "$TRACK_DIR"
  echo ""
  echo "State File:"
  if [[ -f "$TRACK_DIR/metadata.json" ]]; then
    echo "--- metadata.json ---"
    jq '.' "$TRACK_DIR/metadata.json"
    echo ""
    echo "--- metadata.json.generation ---"
    jq '.generation // "MISSING"' "$TRACK_DIR/metadata.json"
    echo ""
    echo "--- metadata.json.beads ---"
    jq '.beads // "MISSING"' "$TRACK_DIR/metadata.json"
  else
    echo "--- metadata.json: MISSING ---"
  fi
  echo ""
  echo "Issues Found:"
  # All checks above run but use "DIAGNOSE:" prefix and skip writes
fi
```

**CRITICAL:** In diagnose mode, all auto-fix and auto-create operations are skipped. Only reporting happens.

## Audit Trail

All repairs logged to `metadata.json.repairs[]`:

```json
{
  "repairs": [
    {
      "timestamp": "2025-12-24T10:30:00Z",
      "action": "auto-fix",
      "field": "track_id",
      "from": "old-track-id_20251223",
      "to": "new-track-id_20251224",
      "by": "T-019b4cec-3f99-70ad-a0c6-617e57d5c0ad"
    }
  ]
}
```

Keep last 10 entries. See `snippets.md` for implementation.

## Step 0.8: Session Lock Detection

Detect concurrent sessions on the same track to prevent conflicts.

```bash
SESSION_LOCK="$TRACK_DIR/.session-lock.json"

if [[ -f "$SESSION_LOCK" ]]; then
  LOCK_AGENT=$(jq -r '.agentId // "unknown"' "$SESSION_LOCK")
  LOCK_TIME=$(jq -r '.lockedAt // "unknown"' "$SESSION_LOCK")
  LAST_HEARTBEAT=$(jq -r '.lastHeartbeat // empty' "$SESSION_LOCK")
  
  # Check heartbeat freshness (stale if > 10 min ago)
  if [[ -n "$LAST_HEARTBEAT" ]]; then
    HEARTBEAT_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$LAST_HEARTBEAT" +%s 2>/dev/null || \
                      date -d "$LAST_HEARTBEAT" +%s 2>/dev/null || echo 0)
    NOW_EPOCH=$(date +%s)
    STALE_THRESHOLD=$((NOW_EPOCH - 600))  # 10 minutes
    
    if [[ $HEARTBEAT_EPOCH -lt $STALE_THRESHOLD && $HEARTBEAT_EPOCH -gt 0 ]]; then
      # Stale lock - auto-unlock
      echo "WARN: Stale session lock detected (no heartbeat for >10 min)"
      echo "      Agent: $LOCK_AGENT"
      echo "      Locked: $LOCK_TIME"
      echo "      Last heartbeat: $LAST_HEARTBEAT"
      
      if [[ $DIAGNOSE_MODE -eq 1 ]]; then
        echo "DIAGNOSE: Would auto-remove stale session lock"
      else
        echo "Auto-removing stale session lock"
        rm "$SESSION_LOCK"
      fi
    else
      # Active lock - prompt user
      echo ""
      echo "⚠️  ACTIVE SESSION DETECTED"
      echo "   Track: $TRACK_ID"
      echo "   Agent: $LOCK_AGENT"
      echo "   Locked: $LOCK_TIME"
      echo "   Last heartbeat: $LAST_HEARTBEAT"
      echo ""
      echo "Options:"
      echo "  [C]ontinue - Proceed anyway (risk conflicts)"
      echo "  [W]ait - Wait for other session to finish"
      echo "  [F]orce - Force unlock (other session will error)"
      echo ""
      # Require explicit user action
    fi
  fi
fi
```

**Heartbeat Protocol:**
- Active sessions update `.session-lock.json.lastHeartbeat` every 5 minutes
- Lock considered stale if heartbeat > 10 minutes ago
- Auto-remove stale locks on detection

**Lock File Schema:**
```json
{
  "agentId": "T-abc123",
  "lockedAt": "2025-12-25T10:00:00Z",
  "lastHeartbeat": "2025-12-25T10:25:00Z",
  "trackId": "beads-integration_20251225",
  "pid": 12345
}
```

**See also:** [Beads Integration](../../beads-integration.md#session-lock) for full session lock protocol.

---

## HALT Conditions

These require manual intervention:

- Corrupted JSON files
- spec.md XOR plan.md (one without the other)
- Content files older than 30 days with missing metadata.json
- track_id mismatch in content file headers
- Active session lock with recent heartbeat (unless user forces)

## PASS Conditions

Validation succeeds when:

- All JSON files parse correctly
- File existence matrix is valid
- track_id matches in metadata.json
- No unresolved HALT conditions
