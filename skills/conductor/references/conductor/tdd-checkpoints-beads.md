# TDD Checkpoints Beads Workflow

**Purpose:** Track RED/GREEN/REFACTOR phases in Beads during TDD-driven implementation. Enabled by default. Use `--no-tdd` to disable.

---

## Overview

By default (unless `--no-tdd` is provided) on `/conductor-implement`, this workflow tracks TDD phase transitions in Beads:

1. **RED** - Test written, fails
2. **GREEN** - Test passes
3. **REFACTOR** - Code cleaned up

Each phase transition updates:
- metadata.json (`tdd_phase` field)
- Bead notes (checkpoint format)

---

## Prerequisites

- Session established (preflight complete)
- Task claimed (`bound_bead` set in metadata.json)
- `/conductor-implement` called without `--no-tdd` flag

---

## Enabling TDD Checkpoints

### Command Usage

```bash
/conductor-implement <track-id>
```

### Flag Detection

```bash
TDD_ENABLED=true
for arg in "$@"; do
  if [[ "$arg" == "--no-tdd" ]]; then
    TDD_ENABLED=false
  fi
done
```

---

## Skip Logic

TDD checkpoints are skipped when:

| Condition | Action |
|-----------|--------|
| `--no-tdd` flag provided | Skip all checkpoints |
| No test files detected | Skip with warning |
| Task is documentation-only | Skip silently |
| Task type is `docs` or `chore` | Skip silently |

### Test File Detection

```bash
detect_test_files() {
  # Common test file patterns
  PATTERNS=(
    "*_test.*"
    "*.test.*"
    "test_*"
    "*_spec.*"
    "*.spec.*"
    "spec_*"
  )
  
  for pattern in "${PATTERNS[@]}"; do
    if find . -name "$pattern" -type f | grep -q .; then
      return 0  # Found test files
    fi
  done
  
  return 1  # No test files
}

if ! detect_test_files; then
  echo "INFO: No test files detected. Skipping TDD checkpoints."
  TDD_ENABLED=false
fi
```

### Documentation Task Detection

```bash
is_docs_task() {
  local TASK_TITLE="$1"
  
  # Check for documentation keywords
  if echo "$TASK_TITLE" | grep -qiE '(document|docs|readme|changelog|tutorial|guide)'; then
    return 0
  fi
  
  # Check bead type
  TASK_TYPE=$(bd show "$TASK_ID" --json | jq -r '.[0].issue_type // "task"')
  if [[ "$TASK_TYPE" == "docs" ]]; then
    return 0
  fi
  
  return 1
}
```

---

## Phase Transitions

### Phase Flow

```text
┌────────────────────────────────────────────────────────────────┐
│                     TDD PHASE FLOW                              │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Task Claimed                                                  │
│        │                                                        │
│        ▼                                                        │
│   ┌─────────────────┐                                           │
│   │  RED PHASE      │ ─── Write failing test                    │
│   │  Checkpoint     │     Test file created OR test fails       │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  GREEN PHASE    │ ─── Make test pass                        │
│   │  Checkpoint     │     Test exits with 0                     │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   ┌─────────────────┐                                           │
│   │  REFACTOR PHASE │ ─── Clean up code                         │
│   │  Checkpoint     │     Code committed                        │
│   └────────┬────────┘                                           │
│            │                                                    │
│            ▼                                                    │
│   Task Complete                                                 │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

### Phase Triggers

| Phase | Trigger Event | Detection Method |
|-------|---------------|------------------|
| RED | Test file created or modified | File watcher or manual signal |
| RED | Test run fails | Exit code != 0 |
| GREEN | Test run passes | Exit code == 0 |
| REFACTOR | Code committed after green | Git commit detected |

---

## Checkpoint Updates

### Update Function

```bash
update_tdd_checkpoint() {
  local TASK_ID="$1"
  local PHASE="$2"  # RED, GREEN, or REFACTOR
  
  # 1. Update metadata.json session state
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  update_metadata_session "tdd_phase" "$PHASE"
  update_metadata_session "heartbeat" "$NOW"
  
  # 2. Build notes message
  case "$PHASE" in
    RED)
      NOTES="IN_PROGRESS: RED phase - writing failing test"
      ;;
    GREEN)
      NOTES="IN_PROGRESS: GREEN phase - making test pass"
      ;;
    REFACTOR)
      NOTES="IN_PROGRESS: REFACTOR phase - cleaning up code"
      ;;
  esac
  
  # 3. Update bead notes
  bd update "$TASK_ID" --notes "$NOTES"
  
  echo "TDD Checkpoint: $PHASE"
}
```

### Notes Format

Each phase has a standardized notes format:

| Phase | Notes Format |
|-------|--------------|
| RED | `IN_PROGRESS: RED phase - writing failing test` |
| GREEN | `IN_PROGRESS: GREEN phase - making test pass` |
| REFACTOR | `IN_PROGRESS: REFACTOR phase - cleaning up code` |

### metadata.json Update

```json
{
  "session": {
    "bound_bead": "my-workflow:3-xyz",
    "tdd_phase": "GREEN",
    "heartbeat": "2025-12-25T12:00:00Z"
  }
}
```

---

## Integration with implement.md

### Workflow Integration

```markdown
## During Task Execution

If TDD enabled (default) and not skipped:

1. **RED Phase**
   - Write failing test
   - Run test → expect failure
   - Call: `update_tdd_checkpoint "$TASK_ID" "RED"`

2. **GREEN Phase**
   - Implement minimal code to pass
   - Run test → expect success
   - Call: `update_tdd_checkpoint "$TASK_ID" "GREEN"`

3. **REFACTOR Phase**
   - Clean up implementation
   - Run test → verify still passes
   - Commit changes
   - Call: `update_tdd_checkpoint "$TASK_ID" "REFACTOR"`
```

### Example Task Flow

```bash
# Task claimed
TASK_ID="my-workflow:3-abc1"

# Check if TDD enabled
if [[ "$TDD_ENABLED" == "true" ]] && ! is_docs_task "$TASK_TITLE"; then
  
  # RED: Write failing test
  echo "Writing test for feature X..."
  # ... create test file ...
  npm test -- --grep "feature X"  # Expect failure
  update_tdd_checkpoint "$TASK_ID" "RED"
  
  # GREEN: Make it pass
  echo "Implementing feature X..."
  # ... write implementation ...
  npm test -- --grep "feature X"  # Expect pass
  update_tdd_checkpoint "$TASK_ID" "GREEN"
  
  # REFACTOR: Clean up
  echo "Refactoring..."
  # ... improve code quality ...
  npm test -- --grep "feature X"  # Verify still passes
  git commit -am "feat: implement feature X"
  update_tdd_checkpoint "$TASK_ID" "REFACTOR"
  
fi

# Close task
bd close "$TASK_ID" --reason completed
```

---

## Metrics Logging

When TDD checkpoints are enabled, log metrics for analysis:

```bash
log_tdd_metric() {
  local TASK_ID="$1"
  local PHASE="$2"
  local DURATION="$3"  # Seconds in phase
  
  METRICS_FILE=".conductor/metrics.jsonl"
  NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  
  echo "{\"event\": \"tdd_cycle\", \"taskId\": \"$TASK_ID\", \"phase\": \"$PHASE\", \"duration\": $DURATION, \"timestamp\": \"$NOW\"}" >> "$METRICS_FILE"
}
```

### Metrics Schema

```json
{
  "event": "tdd_cycle",
  "taskId": "my-workflow:3-abc1",
  "phase": "GREEN",
  "duration": 180,
  "timestamp": "2025-12-25T12:05:00Z"
}
```

---

## Error Handling

| Error | Action |
|-------|--------|
| Session state update fails | Log warning, continue |
| bd update fails | Retry 3x, log warning, continue |
| Test run hangs | Timeout after 5 minutes, prompt user |
| Phase out of order | Log warning, allow (user may know better) |

### Phase Order Validation

```bash
validate_phase_order() {
  local CURRENT_PHASE="$1"
  local NEW_PHASE="$2"
  
  # Define valid transitions
  case "$CURRENT_PHASE:$NEW_PHASE" in
    "":RED|RED:GREEN|GREEN:REFACTOR|REFACTOR:RED)
      return 0  # Valid
      ;;
    *)
      echo "WARN: Unusual phase transition: $CURRENT_PHASE → $NEW_PHASE"
      return 0  # Allow anyway
      ;;
  esac
}
```

---

## Complete Example

### Full TDD Session

```bash
#!/bin/bash
# tdd-session.sh

TASK_ID="$1"
TDD_ENABLED="${TDD_ENABLED:-true}"

# Skip check
if [[ "$TDD_ENABLED" != "true" ]]; then
  echo "TDD checkpoints disabled"
  exit 0
fi

if ! detect_test_files; then
  echo "No test files found, skipping TDD"
  exit 0
fi

TASK_TITLE=$(bd show "$TASK_ID" --json | jq -r '.[0].title')
if is_docs_task "$TASK_TITLE"; then
  echo "Documentation task, skipping TDD"
  exit 0
fi

# Track phase durations
RED_START=$(date +%s)

# RED PHASE
echo "=== RED PHASE ==="
echo "Write a failing test for: $TASK_TITLE"
read -p "Press Enter when test is written and failing..."

update_tdd_checkpoint "$TASK_ID" "RED"
RED_END=$(date +%s)
log_tdd_metric "$TASK_ID" "RED" $((RED_END - RED_START))

# GREEN PHASE
GREEN_START=$(date +%s)
echo ""
echo "=== GREEN PHASE ==="
echo "Implement the minimum code to make the test pass"
read -p "Press Enter when test passes..."

update_tdd_checkpoint "$TASK_ID" "GREEN"
GREEN_END=$(date +%s)
log_tdd_metric "$TASK_ID" "GREEN" $((GREEN_END - GREEN_START))

# REFACTOR PHASE
REFACTOR_START=$(date +%s)
echo ""
echo "=== REFACTOR PHASE ==="
echo "Clean up the code while keeping tests green"
read -p "Press Enter when refactoring is complete and committed..."

update_tdd_checkpoint "$TASK_ID" "REFACTOR"
REFACTOR_END=$(date +%s)
log_tdd_metric "$TASK_ID" "REFACTOR" $((REFACTOR_END - REFACTOR_START))

echo ""
echo "TDD cycle complete for $TASK_ID"
echo "Total time: $((REFACTOR_END - RED_START)) seconds"
```

---

## References

- [Beads Session Workflow](beads-session.md) - Session lifecycle
- [Implement Workflow](../workflows/implement.md) - Task execution
- [Beads Facade](../beads-facade.md) - updateTddPhase API
- [Beads Integration](../beads-integration.md) - Points 4-6
