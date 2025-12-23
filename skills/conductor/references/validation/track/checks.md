# Track Validation Checks

Core validation logic for Conductor tracks. Inline this file in command Phase 0.

## Validation Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Phase 0: Track Validation                    │
├─────────────────────────────────────────────────────────────────────┤
│ 0.1 Resolve track path                                              │
│ 0.2 Check directory exists and not empty                            │
│ 0.3 File existence matrix                                           │
│ 0.4 Validate JSON files (HALT on corruption)                        │
│ 0.5 Auto-create missing state files (if conditions met)             │
│ 0.6 Auto-fix track_id mismatches in state files                     │
│ 0.7 Staleness detection (in_progress state)                         │
└─────────────────────────────────────────────────────────────────────┘
```

## Step 0.1: Resolve Track Path

```bash
TRACK_ID="<provided_track_id>"
TRACK_DIR="conductor/tracks/${TRACK_ID}"

if [[ ! -d "$TRACK_DIR" ]]; then
  TRACK_DIR="conductor/archive/${TRACK_ID}"
fi

if [[ ! -d "$TRACK_DIR" ]]; then
  echo "HALT: Track not found: $TRACK_ID"
  exit 1
fi
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

Check file combinations and determine action:

| design.md | spec.md | plan.md | state files | Action |
|-----------|---------|---------|-------------|--------|
| ✗ | ✗ | ✗ | ✗ | SKIP + warn (empty) |
| ✓ | ✗ | ✗ | ✗ | PASS (design-only state) |
| ✗ | ✓ | ✗ | ✗ | HALT (spec without plan) |
| ✗ | ✗ | ✓ | ✗ | HALT (plan without spec) |
| ✗/✓ | ✓ | ✓ | ✗ | Auto-create state files |
| ✗/✓ | ✓ | ✓ | ✓ | Validate and PASS |

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

```bash
for json_file in "$TRACK_DIR"/*.json "$TRACK_DIR"/.*json; do
  [[ -f "$json_file" ]] || continue
  if ! jq empty "$json_file" 2>/dev/null; then
    echo "HALT: Corrupted JSON: $json_file"
    exit 1
  fi
done
```

## Step 0.5: Auto-Create Missing State Files

When spec.md + plan.md exist but state files are missing:

**Pre-checks (all must pass):**
1. Both files have content (size > 0)
2. Both files < 30 days old
3. No track_id mismatch in headers (if headers exist)

```bash
if [[ $HAS_SPEC -eq 1 && $HAS_PLAN -eq 1 && $HAS_METADATA -eq 0 ]]; then
  # Pre-check 1: Files have content
  if [[ ! -s "$TRACK_DIR/spec.md" || ! -s "$TRACK_DIR/plan.md" ]]; then
    echo "HALT: Empty spec.md or plan.md in $TRACK_ID"
    exit 1
  fi
  
  # Pre-check 2: Files < 30 days old
  THIRTY_DAYS_AGO=$(date -v-30d +%s 2>/dev/null || date -d '30 days ago' +%s)
  SPEC_MTIME=$(stat -f %m "$TRACK_DIR/spec.md" 2>/dev/null || stat -c %Y "$TRACK_DIR/spec.md")
  if [[ $SPEC_MTIME -lt $THIRTY_DAYS_AGO ]]; then
    echo "HALT: spec.md older than 30 days, manual review required"
    exit 1
  fi
  
  # Pre-check 3: Header track_id (if present) matches directory
  HEADER_ID=$(grep -m1 '^# .*Track:' "$TRACK_DIR/spec.md" | sed 's/.*Track: *//' | tr -d ' ')
  if [[ -n "$HEADER_ID" && "$HEADER_ID" != "$TRACK_ID" ]]; then
    echo "WARN: Header track_id mismatch: $HEADER_ID vs $TRACK_ID"
    echo "HALT: Manual review required for track_id mismatch in content files"
    exit 1
  fi
  
  # All pre-checks passed, create state files
  echo "Auto-creating state files for $TRACK_ID"
  # See snippets.md for templates
fi
```

## Step 0.6: Auto-Fix track_id Mismatches

Directory name is source of truth. Auto-fix mismatches in state files.

```bash
# Fix metadata.json
if [[ -f "$TRACK_DIR/metadata.json" ]]; then
  CURRENT_ID=$(jq -r '.track_id // empty' "$TRACK_DIR/metadata.json")
  if [[ -n "$CURRENT_ID" && "$CURRENT_ID" != "$TRACK_ID" ]]; then
    echo "Auto-fixing track_id in metadata.json: $CURRENT_ID → $TRACK_ID"
    jq --arg id "$TRACK_ID" '.track_id = $id' "$TRACK_DIR/metadata.json" > "$TRACK_DIR/metadata.json.tmp"
    mv "$TRACK_DIR/metadata.json.tmp" "$TRACK_DIR/metadata.json"
    # Log repair (see snippets.md for audit trail)
  fi
fi

# Fix .track-progress.json
if [[ -f "$TRACK_DIR/.track-progress.json" ]]; then
  CURRENT_ID=$(jq -r '.trackId // empty' "$TRACK_DIR/.track-progress.json")
  if [[ -n "$CURRENT_ID" && "$CURRENT_ID" != "$TRACK_ID" ]]; then
    echo "Auto-fixing trackId in .track-progress.json: $CURRENT_ID → $TRACK_ID"
    jq --arg id "$TRACK_ID" '.trackId = $id' "$TRACK_DIR/.track-progress.json" > "$TRACK_DIR/.track-progress.json.tmp"
    mv "$TRACK_DIR/.track-progress.json.tmp" "$TRACK_DIR/.track-progress.json"
  fi
fi

# Fix .fb-progress.json
if [[ -f "$TRACK_DIR/.fb-progress.json" ]]; then
  CURRENT_ID=$(jq -r '.trackId // empty' "$TRACK_DIR/.fb-progress.json")
  if [[ -n "$CURRENT_ID" && "$CURRENT_ID" != "$TRACK_ID" ]]; then
    echo "Auto-fixing trackId in .fb-progress.json: $CURRENT_ID → $TRACK_ID"
    jq --arg id "$TRACK_ID" '.trackId = $id' "$TRACK_DIR/.fb-progress.json" > "$TRACK_DIR/.fb-progress.json.tmp"
    mv "$TRACK_DIR/.fb-progress.json.tmp" "$TRACK_DIR/.fb-progress.json"
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

When `.fb-progress.json.status = "in_progress"`, warn user.

```bash
if [[ -f "$TRACK_DIR/.fb-progress.json" ]]; then
  FB_STATUS=$(jq -r '.status // empty' "$TRACK_DIR/.fb-progress.json")
  if [[ "$FB_STATUS" == "in_progress" ]]; then
    STARTED_AT=$(jq -r '.startedAt // "unknown"' "$TRACK_DIR/.fb-progress.json")
    THREAD_ID=$(jq -r '.threadId // "unknown"' "$TRACK_DIR/.fb-progress.json")
    echo ""
    echo "⚠️  STALE IN_PROGRESS DETECTED"
    echo "   Track: $TRACK_ID"
    echo "   Started: $STARTED_AT"
    echo "   Thread: $THREAD_ID"
    echo ""
    echo "Options:"
    echo "  [R]esume - Continue from last checkpoint"
    echo "  [X]Reset - Clear progress and start fresh"
    echo "  [D]iagnose - Show detailed state"
    echo ""
    # Never auto-reset - require explicit user action
    # In --diagnose mode, just report and continue
  fi
fi
```

## Diagnose Mode

When `--diagnose` flag is set, report all issues without making changes:

```bash
DIAGNOSE_MODE=0
[[ "$1" == "--diagnose" ]] && DIAGNOSE_MODE=1

if [[ $DIAGNOSE_MODE -eq 1 ]]; then
  echo "=== Track Validation Report: $TRACK_ID ==="
  echo ""
  echo "Files:"
  ls -la "$TRACK_DIR"
  echo ""
  echo "State Files:"
  for f in metadata.json .track-progress.json .fb-progress.json; do
    if [[ -f "$TRACK_DIR/$f" ]]; then
      echo "--- $f ---"
      jq '.' "$TRACK_DIR/$f"
    else
      echo "--- $f: MISSING ---"
    fi
  done
  echo ""
  echo "Issues Found:"
  # (run checks above but collect issues instead of fixing)
fi
```

## Audit Trail

All repairs logged to `metadata.json.repairs[]`:

```json
{
  "repairs": [
    {
      "timestamp": "2024-12-24T10:30:00Z",
      "action": "auto-fix",
      "field": "track_id",
      "from": "old-track-id_20241223",
      "to": "new-track-id_20241224",
      "by": "T-019b4cec-3f99-70ad-a0c6-617e57d5c0ad"
    }
  ]
}
```

Keep last 10 entries. See `snippets.md` for implementation.

## HALT Conditions

These require manual intervention:
- Corrupted JSON files
- spec.md XOR plan.md (one without the other)
- Content files older than 30 days with missing state files
- track_id mismatch in content file headers

## PASS Conditions

Validation succeeds when:
- All JSON files parse correctly
- File existence matrix is valid
- track_id matches across all state files
- No unresolved HALT conditions
